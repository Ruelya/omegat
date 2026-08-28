//! Java `RemoteRepositoryProvider`.

use crate::error::{Result, TeamError};
use crate::mapping::{copy_mapped_cancellable, effective_mappings, propagate_deleted, CopyDir};
use crate::project_team_settings::{is_inplace, prep_dir};
use crate::rebase_and_commit::rebase_all;
use crate::rebase_utils::save_bases;
use crate::remote_repository_factory;
use crate::team_settings::{clear_resolved, save_conflicts};
use crate::transaction_envelope::{
    normalized, write_json_atomic, TransactionCommit, TransactionEnvelope, TransactionStatus,
    REQUEST_CANCELLED_CODE, TRANSACTION_ENVELOPE_VERSION,
};
use crate::{team_enabled, SyncReport};
use fs2::FileExt;
use omegat_core::cancellation::CancellationToken;
use omegat_core::properties::ProjectProperties;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static OWNER_CLAIM_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const PRODUCT_JOURNAL_VERSION: u8 = 2;
#[cfg(test)]
static FAIL_COMMIT_REPOSITORY: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(test)]
static COMMIT_FAULT_INJECTION_LOCK: Mutex<()> = Mutex::new(());
#[cfg(test)]
static CRASH_AFTER_PUBLISH_REPOSITORY: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(test)]
static CRASH_AFTER_PRODUCT_COMMIT: AtomicUsize = AtomicUsize::new(0);

fn check_cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(TeamError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) struct ProjectTransactionLock {
    _file: std::fs::File,
    waited: bool,
}

pub(crate) fn acquire_project_transaction_lock(
    props: &ProjectProperties,
) -> Result<ProjectTransactionLock> {
    let dir = transaction_dir(props);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("operation.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)?;
    file.try_lock_exclusive().map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            TeamError::Conflict(format!(
                "team project is locked by another process: {}",
                props.root.display()
            ))
        } else {
            TeamError::Io(error)
        }
    })?;
    Ok(ProjectTransactionLock {
        _file: file,
        waited: false,
    })
}

fn wait_for_project_transaction_lock(props: &ProjectProperties) -> Result<ProjectTransactionLock> {
    let dir = transaction_dir(props);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("operation.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)?;
    let waited = match file.try_lock_exclusive() {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            resolve_cancellation_lock_checkpoint(
                "OMEGAT_TEST_RESOLVE_CANCELLATION_WAIT_MARKER",
                props,
                "waiting-for-owner-lock",
            )?;
            file.lock_exclusive().map_err(TeamError::Io)?;
            true
        }
        Err(error) => return Err(TeamError::Io(error)),
    };
    Ok(ProjectTransactionLock {
        _file: file,
        waited,
    })
}

#[cfg(test)]
pub(crate) fn fail_next_commit_for(repository_index: usize) {
    FAIL_COMMIT_REPOSITORY.store(repository_index, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn lock_commit_fault_injection() -> MutexGuard<'static, ()> {
    COMMIT_FAULT_INJECTION_LOCK.lock().unwrap()
}

#[cfg(test)]
pub(crate) fn crash_after_publish_for(repository_index: usize) {
    CRASH_AFTER_PUBLISH_REPOSITORY.store(repository_index, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn crash_after_product_commit() {
    CRASH_AFTER_PRODUCT_COMMIT.store(1, Ordering::SeqCst);
}

fn commit_repository(
    props: &ProjectProperties,
    repository_index: usize,
    on_versions: &[Option<String>],
    comment: &str,
) -> Result<Option<String>> {
    #[cfg(test)]
    if FAIL_COMMIT_REPOSITORY
        .compare_exchange(
            repository_index,
            usize::MAX,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok()
    {
        return Err(TeamError::Command(format!(
            "injected repository {repository_index} commit failure"
        )));
    }
    remote_repository_factory::commit_after_versions(
        props,
        &props.repositories[repository_index],
        on_versions,
        comment,
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileRemoteSnapshot {
    repository_index: usize,
    source: PathBuf,
    backup: PathBuf,
    is_file: bool,
    existed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalProductSnapshot {
    target: PathBuf,
    backup: PathBuf,
    is_file: bool,
    existed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    symlink_target: Option<PathBuf>,
    #[serde(default)]
    symlink_is_dir: bool,
}

struct SyncSnapshot {
    base: PathBuf,
    project: PathBuf,
    prep: PathBuf,
    prep_existed: bool,
    file_remotes: Vec<FileRemoteSnapshot>,
    external_products: Vec<ExternalProductSnapshot>,
}

impl SyncSnapshot {
    fn capture(props: &ProjectProperties, base: PathBuf) -> Result<Self> {
        Self::capture_cancellable(props, base, true, &[], &CancellationToken::default(), None)
    }

    fn capture_cancellable(
        props: &ProjectProperties,
        base: PathBuf,
        include_file_remotes: bool,
        external_products: &[PathBuf],
        cancellation: &CancellationToken,
        checkpoint: Option<&'static str>,
    ) -> Result<Self> {
        if base.exists() {
            std::fs::remove_dir_all(&base)?;
        }
        let project = base.join("project");
        if let Err(error) = crate::team_utils::copy_tree_cancellable(
            &props.root,
            &project,
            true,
            cancellation,
            checkpoint,
        ) {
            let _ = remove_path(&base);
            return Err(error);
        }

        let prep_source = prep_dir(props);
        let prep = base.join("prep");
        let prep_existed = prep_source.exists();
        if prep_existed {
            if let Err(error) = crate::team_utils::copy_tree_cancellable(
                &prep_source,
                &prep,
                false,
                cancellation,
                checkpoint,
            ) {
                let _ = remove_path(&base);
                return Err(error);
            }
        }

        let mut file_remotes = Vec::new();
        for (repository_index, repo) in props
            .repositories
            .iter()
            .enumerate()
            .filter(|_| include_file_remotes)
        {
            if repo.repo_type != "file" || is_inplace(props, repo) {
                continue;
            }
            let source = PathBuf::from(&repo.url);
            let backup = base.join("file-remotes").join(repository_index.to_string());
            let existed = source.exists();
            let is_file = source.is_file();
            if existed && is_file {
                if let Some(parent) = backup.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&source, &backup)?;
            } else if existed {
                crate::team_utils::copy_tree(&source, &backup, false)?;
            }
            file_remotes.push(FileRemoteSnapshot {
                repository_index,
                source,
                backup,
                is_file,
                existed,
            });
        }

        let mut external_snapshots = Vec::new();
        let mut captured_targets = BTreeSet::new();
        for (index, target) in external_products.iter().enumerate() {
            check_cancelled(cancellation)?;
            let target = if target.is_absolute() {
                target.clone()
            } else {
                props.root.join(target)
            };
            if target.starts_with(&props.root) || !captured_targets.insert(target.clone()) {
                continue;
            }
            let backup = base.join("external-products").join(index.to_string());
            let existed = target.exists() || target.is_symlink();
            let symlink_target = target
                .is_symlink()
                .then(|| std::fs::read_link(&target))
                .transpose()?;
            let symlink_is_dir = symlink_target.is_some() && target.is_dir();
            let is_file = target.is_file() && symlink_target.is_none();
            if existed && is_file {
                if let Some(parent) = backup.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&target, &backup)?;
            } else if existed && symlink_target.is_none() {
                crate::team_utils::copy_tree(&target, &backup, false)?;
            }
            external_snapshots.push(ExternalProductSnapshot {
                target,
                backup,
                is_file,
                existed,
                symlink_target,
                symlink_is_dir,
            });
        }

        sync_snapshot_tree(&base)?;
        Ok(Self {
            base,
            project,
            prep,
            prep_existed,
            file_remotes,
            external_products: external_snapshots,
        })
    }

    fn open(
        props: &ProjectProperties,
        base: PathBuf,
        prep_existed: bool,
        file_remotes: Vec<FileRemoteSnapshot>,
        external_products: Vec<ExternalProductSnapshot>,
    ) -> Result<Self> {
        let project = base.join("project");
        if !project.is_dir() {
            return Err(TeamError::Command(format!(
                "team transaction snapshot is missing at {}",
                project.display()
            )));
        }
        let prep = base.join("prep");
        for snapshot in &file_remotes {
            let repo = props
                .repositories
                .get(snapshot.repository_index)
                .ok_or_else(|| {
                    TeamError::Command(format!(
                        "team transaction repository {} is no longer configured",
                        snapshot.repository_index
                    ))
                })?;
            if repo.repo_type != "file" || Path::new(&repo.url) != snapshot.source {
                return Err(TeamError::Command(format!(
                    "team transaction repository {} changed configuration",
                    snapshot.repository_index
                )));
            }
        }
        Ok(Self {
            base,
            project,
            prep,
            prep_existed,
            file_remotes,
            external_products,
        })
    }

    fn restore_project_and_prep(&self, props: &ProjectProperties) -> Result<()> {
        for entry in std::fs::read_dir(&props.root)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == ".repositories" || name == ".git" || name == ".svn" {
                continue;
            }
            remove_path(&entry.path())?;
        }
        crate::team_utils::copy_tree(&self.project, &props.root, true)?;

        let prep_target = prep_dir(props);
        remove_path(&prep_target)?;
        if self.prep_existed {
            crate::team_utils::copy_tree(&self.prep, &prep_target, false)?;
        }
        for snapshot in &self.external_products {
            remove_path(&snapshot.target)?;
            if !snapshot.existed {
                continue;
            }
            if let Some(link_target) = &snapshot.symlink_target {
                restore_symlink(link_target, &snapshot.target, snapshot.symlink_is_dir)?;
            } else if snapshot.is_file {
                if let Some(parent) = snapshot.target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&snapshot.backup, &snapshot.target)?;
            } else {
                crate::team_utils::copy_tree(&snapshot.backup, &snapshot.target, false)?;
            }
        }
        Ok(())
    }

    fn restore_project_and_prep_durable(&self, props: &ProjectProperties) -> Result<()> {
        self.restore_project_and_prep(props)?;
        sync_restored_project_and_prep(props, &self.external_products)
    }

    fn restore_file_remote(&self, repository_index: usize) -> Result<()> {
        let Some(snapshot) = self
            .file_remotes
            .iter()
            .find(|snapshot| snapshot.repository_index == repository_index)
        else {
            return Ok(());
        };
        remove_path(&snapshot.source)?;
        if !snapshot.existed {
            return Ok(());
        }
        if snapshot.is_file {
            if let Some(parent) = snapshot.source.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&snapshot.backup, &snapshot.source)?;
        } else {
            crate::team_utils::copy_tree(&snapshot.backup, &snapshot.source, false)?;
        }
        Ok(())
    }
}

fn restore_symlink(link_target: &Path, path: &Path, target_is_dir: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        let _ = target_is_dir;
        std::os::unix::fs::symlink(link_target, path)?;
    }
    #[cfg(windows)]
    {
        if target_is_dir {
            std::os::windows::fs::symlink_dir(link_target, path)?;
        } else {
            std::os::windows::fs::symlink_file(link_target, path)?;
        }
    }
    Ok(())
}

fn sync_snapshot_tree(root: &Path) -> Result<()> {
    let mut directories = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| {
            TeamError::Command(format!(
                "walk transaction snapshot {}: {error}",
                root.display()
            ))
        })?;
        if entry.file_type().is_file() {
            std::fs::File::open(entry.path())?.sync_all()?;
        } else if entry.file_type().is_dir() {
            directories.push(entry.path().to_path_buf());
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        std::fs::File::open(directory)?.sync_all()?;
    }
    Ok(())
}

fn sync_restored_project_and_prep(
    props: &ProjectProperties,
    external_products: &[ExternalProductSnapshot],
) -> Result<()> {
    for entry in std::fs::read_dir(&props.root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".repositories" || name == ".git" || name == ".svn" {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            sync_snapshot_tree(&entry.path())?;
        } else if file_type.is_file() {
            File::open(entry.path())?.sync_all()?;
        }
    }
    File::open(&props.root)?.sync_all()?;

    let prep = prep_dir(props);
    if prep.is_dir() {
        sync_snapshot_tree(&prep)?;
    }
    if let Some(repositories) = prep.parent() {
        File::open(repositories)?.sync_all()?;
    }
    for snapshot in external_products {
        if snapshot.target.is_dir() && !snapshot.target.is_symlink() {
            sync_snapshot_tree(&snapshot.target)?;
        } else if snapshot.target.is_file() {
            File::open(&snapshot.target)?.sync_all()?;
        }
        if let Some(parent) = snapshot.target.parent() {
            File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProductFileReceipt {
    path: String,
    kind: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TeamProductManifest {
    files: Vec<ProductFileReceipt>,
    repository_versions: Vec<Option<String>>,
    root_git_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SyncTransactionPayload {
    operation: String,
    phase: String,
    snapshot: PathBuf,
    prep_existed: bool,
    file_remotes: Vec<FileRemoteSnapshot>,
    repository_count: usize,
    rollback_versions: Vec<Option<String>>,
    commit_started: Vec<usize>,
    published: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product_manifest: Option<TeamProductManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_git_rollback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh: Option<TransactionRendererPayload>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    external_products: Vec<ExternalProductSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
struct SyncTransaction(TransactionEnvelope<SyncTransactionPayload>);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProductTransactionJournal {
    version: u8,
    project_root: PathBuf,
    batches: Vec<SyncTransaction>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RendererOwnerClaim {
    version: u8,
    project_root: PathBuf,
    app_instance: String,
    process_id: u32,
    generation: u64,
    claim_id: String,
    updated_unix_ms: u128,
}

/// Small renderer-facing view of any durable product transaction.
///
/// The potentially large product manifest remains in the `active.json` journal; the
/// renderer needs only the envelope identity and receipt fingerprint to
/// explicitly acknowledge that it consumed the committed product result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRendererReceipt {
    pub version: u8,
    pub project_root: PathBuf,
    pub generation: u64,
    pub batch_id: String,
    pub status: TransactionStatus,
    pub error_code: Option<i32>,
    pub updated_unix_ms: u128,
    pub payload: TransactionRendererPayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<TransactionCommit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRendererPayload {
    #[serde(default = "external_refresh_operation")]
    pub operation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fingerprints: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_result: Option<serde_json::Value>,
}

fn external_refresh_operation() -> String {
    "project.external-refresh".into()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRendererAck {
    pub version: u8,
    pub project_root: PathBuf,
    pub generation: u64,
    pub batch_id: String,
    pub acknowledged: bool,
    pub already_acknowledged: bool,
}

impl std::ops::Deref for SyncTransaction {
    type Target = SyncTransactionPayload;

    fn deref(&self) -> &Self::Target {
        &self.0.payload
    }
}

impl std::ops::DerefMut for SyncTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.payload
    }
}

impl SyncTransaction {
    fn is_refresh(&self) -> bool {
        self.refresh.is_some()
    }

    fn ensure_slot_available(props: &ProjectProperties) -> Result<()> {
        if let Some(transaction) = Self::load_active_operation(props)? {
            return Err(TeamError::Conflict(format!(
                "team transaction {} is still in progress",
                transaction.0.batch_id
            )));
        }
        Ok(())
    }

    fn begin(
        props: &ProjectProperties,
        operation: &str,
        generation: u64,
        batch_id: Option<&str>,
    ) -> Result<(Self, SyncSnapshot)> {
        Self::ensure_slot_available(props)?;
        let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let generated_id = format!("{}-{}-{sequence}", unix_ms(), std::process::id());
        let id = batch_id
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_id.clone());
        let snapshot_path = transaction_dir(props).join(format!("{generated_id}.snapshot"));
        let snapshot = SyncSnapshot::capture(props, snapshot_path.clone())?;
        let mut transaction = Self(TransactionEnvelope::pending(
            &props.root,
            generation,
            id,
            SyncTransactionPayload {
                operation: operation.into(),
                phase: "captured".into(),
                snapshot: snapshot_path,
                prep_existed: snapshot.prep_existed,
                file_remotes: snapshot.file_remotes.clone(),
                repository_count: props.repositories.len(),
                rollback_versions: vec![None; props.repositories.len()],
                commit_started: Vec::new(),
                published: Vec::new(),
                product_manifest: None,
                root_git_rollback: None,
                refresh: None,
                external_products: Vec::new(),
            },
        ));
        transaction.persist(props)?;
        Ok((transaction, snapshot))
    }

    fn begin_local_cancellable(
        props: &ProjectProperties,
        operation: &str,
        cancellation: &CancellationToken,
        checkpoint: &'static str,
        generation: u64,
        batch_id: Option<&str>,
        external_products: &[PathBuf],
    ) -> Result<(Self, SyncSnapshot)> {
        Self::ensure_slot_available(props)?;
        let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let generated_id = format!("{}-{}-{sequence}", unix_ms(), std::process::id());
        let id = batch_id
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_id.clone());
        let snapshot_path = transaction_dir(props).join(format!("{generated_id}.snapshot"));
        let mut transaction = Self(TransactionEnvelope::pending(
            &props.root,
            generation,
            id,
            SyncTransactionPayload {
                operation: operation.into(),
                phase: "capturing".into(),
                snapshot: snapshot_path.clone(),
                prep_existed: prep_dir(props).exists(),
                file_remotes: Vec::new(),
                repository_count: props.repositories.len(),
                rollback_versions: vec![None; props.repositories.len()],
                commit_started: Vec::new(),
                published: Vec::new(),
                product_manifest: None,
                root_git_rollback: None,
                refresh: None,
                external_products: Vec::new(),
            },
        ));
        transaction.persist(props)?;
        let snapshot = match SyncSnapshot::capture_cancellable(
            props,
            snapshot_path,
            false,
            external_products,
            cancellation,
            Some(checkpoint),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                transaction.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
        };
        transaction.prep_existed = snapshot.prep_existed;
        transaction.external_products = snapshot.external_products.clone();
        transaction.phase = "captured".into();
        transaction.persist(props)?;
        Ok((transaction, snapshot))
    }

    fn persist(&mut self, props: &ProjectProperties) -> Result<()> {
        self.0.touch();
        self.persist_current(props)
    }

    fn persist_preserving_dispatch_order(&mut self, props: &ProjectProperties) -> Result<()> {
        self.persist_current(props)
    }

    fn persist_current(&mut self, props: &ProjectProperties) -> Result<()> {
        self.0
            .validate_for_root(&props.root)
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        let dir = transaction_dir(props);
        std::fs::create_dir_all(&dir)?;
        let mut journal = load_product_journal(props)?;
        if let Some(existing) = journal
            .batches
            .iter_mut()
            .find(|transaction| transaction.0.batch_id == self.0.batch_id)
        {
            *existing = self.clone();
        } else {
            journal.batches.push(self.clone());
        }
        write_product_journal(props, &journal)?;
        let mut history = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("history.ndjson"))?;
        serde_json::to_writer(&mut history, self)
            .map_err(|error| TeamError::Command(format!("team transaction history: {error}")))?;
        history.write_all(b"\n")?;
        history.sync_all()?;
        Ok(())
    }

    fn finish(
        mut self,
        props: &ProjectProperties,
        phase: &str,
        status: TransactionStatus,
        error_code: Option<i32>,
    ) -> Result<()> {
        self.phase = phase.into();
        self.0.transition(status, error_code);
        self.persist(props)?;
        self.remove_from_journal(props)
    }

    fn publish_product_commit(
        &mut self,
        props: &ProjectProperties,
        phase: &str,
        await_renderer_ack: bool,
    ) -> Result<()> {
        let manifest = capture_product_manifest(props, &self.external_products)?;
        let manifest_items = manifest.files.len() as u64
            + manifest.repository_versions.len() as u64
            + u64::from(manifest.root_git_version.is_some());
        self.phase = phase.into();
        self.product_manifest = Some(manifest.clone());
        self.0
            .commit_product(
                if await_renderer_ack {
                    TransactionStatus::SidecarCommitted
                } else {
                    TransactionStatus::Completed
                },
                &manifest,
                manifest_items,
            )
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        self.persist(props)?;
        #[cfg(test)]
        if CRASH_AFTER_PRODUCT_COMMIT.swap(0, Ordering::SeqCst) == 1 {
            std::process::abort();
        }
        Ok(())
    }

    fn renderer_receipt(&self) -> Result<TransactionRendererReceipt> {
        if self.0.status != TransactionStatus::SidecarCommitted
            && !(self.is_refresh() && self.0.status == TransactionStatus::Pending)
        {
            return Err(TeamError::Command(format!(
                "transaction {} is not awaiting renderer dispatch",
                self.0.batch_id
            )));
        }
        let commit = self.0.commit.clone();
        if self.0.status == TransactionStatus::SidecarCommitted && commit.is_none() {
            return Err(TeamError::Command(format!(
                "transaction {} has no product receipt",
                self.0.batch_id
            )));
        }
        Ok(TransactionRendererReceipt {
            version: self.0.version,
            project_root: self.0.project_root.clone(),
            generation: self.0.generation,
            batch_id: self.0.batch_id.clone(),
            status: self.0.status,
            error_code: self.0.error_code,
            updated_unix_ms: self.0.updated_unix_ms,
            payload: self
                .refresh
                .clone()
                .unwrap_or_else(|| TransactionRendererPayload {
                    operation: self.operation.clone(),
                    paths: Vec::new(),
                    fingerprints: BTreeMap::new(),
                    sources: Vec::new(),
                    committed_result: None,
                }),
            commit,
        })
    }

    fn validate_repository_shape(&self, props: &ProjectProperties) -> Result<()> {
        if self.is_refresh()
            || !matches!(
                self.operation.as_str(),
                "sync" | "commit-source" | "commit-target" | "resolve-conflict"
            )
        {
            return Ok(());
        }
        if self.repository_count != props.repositories.len()
            || self.rollback_versions.len() != props.repositories.len()
        {
            return Err(TeamError::Command(format!(
                "team transaction {} expected {} repositories, found {}",
                self.0.batch_id,
                self.repository_count,
                props.repositories.len()
            )));
        }
        Ok(())
    }

    fn cleanup(self, props: &ProjectProperties) -> Result<()> {
        self.remove_from_journal(props)
    }

    fn finish_for_error(
        self,
        props: &ProjectProperties,
        phase: &str,
        error: &TeamError,
    ) -> Result<()> {
        if matches!(error, TeamError::Cancelled) {
            self.finish(
                props,
                phase,
                TransactionStatus::RequestCancelled,
                Some(REQUEST_CANCELLED_CODE),
            )
        } else {
            self.finish(props, phase, TransactionStatus::Cancelled, None)
        }
    }

    fn validate_loaded(&self, props: &ProjectProperties) -> Result<()> {
        let transaction = self;
        transaction
            .0
            .validate_for_root(&props.root)
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        if let Some(refresh) = &transaction.refresh {
            if transaction.operation != "project.external-refresh"
                || refresh.operation != transaction.operation
                || refresh.paths.is_empty()
                || refresh.sources.is_empty()
                || !refresh
                    .sources
                    .iter()
                    .all(|source| matches!(source.as_str(), "native" | "sidecar"))
            {
                return Err(TeamError::Command(format!(
                    "invalid refresh transaction {}",
                    transaction.0.batch_id
                )));
            }
            match transaction.0.status {
                TransactionStatus::Pending if refresh.committed_result.is_some() => {
                    return Err(TeamError::Command(format!(
                        "pending refresh {} carries a committed result",
                        transaction.0.batch_id
                    )));
                }
                TransactionStatus::SidecarCommitted => {
                    let result = refresh.committed_result.as_ref().ok_or_else(|| {
                        TeamError::Command(format!(
                            "sidecar-committed refresh {} has no durable result",
                            transaction.0.batch_id
                        ))
                    })?;
                    let items = refresh_result_items(result);
                    if !transaction.0.verify_product(result, items) {
                        return Err(TeamError::Command(format!(
                            "refresh transaction {} product receipt mismatch",
                            transaction.0.batch_id
                        )));
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if matches!(
            transaction.0.status,
            TransactionStatus::SidecarCommitted | TransactionStatus::Completed
        ) {
            let manifest = transaction.product_manifest.as_ref().ok_or_else(|| {
                TeamError::Command(format!(
                    "team transaction {} has no product manifest",
                    transaction.0.batch_id
                ))
            })?;
            let manifest_items = manifest.files.len() as u64
                + manifest.repository_versions.len() as u64
                + u64::from(manifest.root_git_version.is_some());
            if !transaction.0.verify_product(manifest, manifest_items) {
                return Err(TeamError::Command(format!(
                    "team transaction {} product receipt mismatch",
                    transaction.0.batch_id
                )));
            }
        }
        Ok(())
    }

    fn load_active_operation(props: &ProjectProperties) -> Result<Option<Self>> {
        let journal = load_product_journal(props)?;
        let mut pending = journal.batches.into_iter().filter(|transaction| {
            !transaction.is_refresh()
                && matches!(
                    transaction.0.status,
                    TransactionStatus::Pending | TransactionStatus::CancellationPending
                )
        });
        let transaction = pending.next();
        if pending.next().is_some() {
            return Err(TeamError::Command(
                "product transaction journal contains multiple active operations".into(),
            ));
        }
        Ok(transaction)
    }

    fn load_dispatch_head(props: &ProjectProperties) -> Result<Option<Self>> {
        Ok(load_product_journal(props)?
            .batches
            .into_iter()
            .find(|transaction| {
                transaction.0.status == TransactionStatus::SidecarCommitted
                    || (transaction.is_refresh()
                        && transaction.0.status == TransactionStatus::Pending)
            }))
    }

    fn load_product_receipt_head(props: &ProjectProperties) -> Result<Option<Self>> {
        Ok(load_product_journal(props)?
            .batches
            .into_iter()
            .find(|transaction| {
                !transaction.is_refresh()
                    && transaction.0.status == TransactionStatus::SidecarCommitted
            }))
    }

    fn load_receipt(props: &ProjectProperties, batch_id: &str) -> Result<Option<Self>> {
        Ok(load_product_journal(props)?
            .batches
            .into_iter()
            .find(|transaction| {
                transaction.0.batch_id == batch_id
                    && transaction.0.status == TransactionStatus::SidecarCommitted
            }))
    }

    fn remove_from_journal(self, props: &ProjectProperties) -> Result<()> {
        let mut journal = load_product_journal(props)?;
        let before = journal.batches.len();
        journal
            .batches
            .retain(|transaction| transaction.0.batch_id != self.0.batch_id);
        if journal.batches.len() == before {
            return Err(TeamError::Command(format!(
                "product transaction {} disappeared from journal",
                self.0.batch_id
            )));
        }
        if !self.snapshot.as_os_str().is_empty() {
            remove_path(&self.snapshot)?;
        }
        if journal.batches.is_empty() {
            remove_path(&transaction_dir(props).join("active.json"))?;
            remove_path(&transaction_dir(props).join(".active.previous.json"))?;
        } else {
            write_product_journal(props, &journal)?;
        }
        Ok(())
    }
}

fn load_product_journal(props: &ProjectProperties) -> Result<ProductTransactionJournal> {
    let dir = transaction_dir(props);
    let active = dir.join("active.json");
    let previous = dir.join(".active.previous.json");
    let path = if active.is_file() {
        active
    } else if previous.is_file() {
        previous
    } else {
        return Ok(ProductTransactionJournal {
            version: PRODUCT_JOURNAL_VERSION,
            project_root: normalized(&props.root),
            batches: Vec::new(),
        });
    };
    let bytes = std::fs::read(&path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| TeamError::Command(format!("team transaction journal: {error}")))?;
    let journal = if value.get("batches").is_some() {
        serde_json::from_value::<ProductTransactionJournal>(value)
            .map_err(|error| TeamError::Command(format!("team transaction journal: {error}")))?
    } else {
        // Version-1 installations stored one transparent envelope directly in
        // active.json. Read it as the first journal row without rewriting it
        // until the next durable state transition.
        let transaction = serde_json::from_value::<SyncTransaction>(value)
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        ProductTransactionJournal {
            version: PRODUCT_JOURNAL_VERSION,
            project_root: normalized(&props.root),
            batches: vec![transaction],
        }
    };
    if journal.version != PRODUCT_JOURNAL_VERSION {
        return Err(TeamError::Command(format!(
            "unsupported product transaction journal version {}",
            journal.version
        )));
    }
    if normalized(&journal.project_root) != normalized(&props.root) {
        return Err(TeamError::Command(format!(
            "product transaction journal root {} does not match {}",
            journal.project_root.display(),
            props.root.display()
        )));
    }
    for transaction in &journal.batches {
        transaction.validate_loaded(props)?;
    }
    Ok(journal)
}

fn write_product_journal(
    props: &ProjectProperties,
    journal: &ProductTransactionJournal,
) -> Result<()> {
    write_json_atomic(&transaction_dir(props).join("active.json"), journal)
        .map_err(|error| TeamError::Command(format!("team transaction journal: {error}")))
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        TeamError::Command(format!(
            "product transaction path has no parent: {}",
            path.display()
        ))
    })?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn archive_terminal_product_transactions(
    props: &ProjectProperties,
    terminal: &[SyncTransaction],
) -> Result<()> {
    let path = transaction_dir(props).join("history.ndjson");
    let existing = match std::fs::read_to_string(&path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(TeamError::Io(error)),
    };
    let existing = existing
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .map_err(|error| TeamError::Command(format!("team transaction history: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut history = OpenOptions::new().create(true).append(true).open(&path)?;
    for transaction in terminal {
        let value = serde_json::to_value(transaction)
            .map_err(|error| TeamError::Command(format!("team transaction history: {error}")))?;
        if existing.iter().any(|archived| archived == &value) {
            continue;
        }
        serde_json::to_writer(&mut history, transaction)
            .map_err(|error| TeamError::Command(format!("team transaction history: {error}")))?;
        history.write_all(b"\n")?;
    }
    history.sync_all()?;
    sync_parent(&path)
}

fn product_compaction_checkpoint(point: &str) -> Result<()> {
    if std::env::var("OMEGAT_TEST_PRODUCT_COMPACTION_POINT").as_deref() != Ok(point) {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_PRODUCT_COMPACTION_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(TeamError::Io(error)),
    };
    writeln!(file, "{point}")?;
    file.sync_all()?;
    sync_parent(&marker)?;
    if let Some(release) = std::env::var_os("OMEGAT_TEST_PRODUCT_COMPACTION_RELEASE") {
        let release = PathBuf::from(release);
        while !release.is_file() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        return Ok(());
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn resolve_cancellation_checkpoint_from_env(
    point: &str,
    point_variable: &str,
    trigger_variable: &str,
    marker_variable: &str,
    release_variable: &str,
) -> Result<()> {
    if std::env::var(point_variable).as_deref() != Ok(point) {
        return Ok(());
    }
    if let Some(trigger) = std::env::var_os(trigger_variable) {
        if !PathBuf::from(trigger).is_file() {
            return Ok(());
        }
    }
    let Some(marker) = std::env::var_os(marker_variable) else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(TeamError::Io(error)),
    };
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({
            "point": point,
            "sidecar_process_id": std::process::id(),
        }),
    )
    .map_err(|error| TeamError::Command(format!("resolve cancellation checkpoint: {error}")))?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    sync_parent(&marker)?;
    if let Some(release) = std::env::var_os(release_variable) {
        let release = PathBuf::from(release);
        while !release.is_file() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        return Ok(());
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn resolve_cancellation_checkpoint(point: &str) -> Result<()> {
    resolve_cancellation_checkpoint_from_env(
        point,
        "OMEGAT_TEST_RESOLVE_CANCELLATION_POINT",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_TRIGGER",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_MARKER",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_RELEASE",
    )?;
    // A single process can own two successive durable boundaries. Keeping a
    // separate follow-up marker lets crash tests stop the rollback publisher
    // and then the terminal publisher while all later callers remain blocked
    // in their original OS-lock wait.
    resolve_cancellation_checkpoint_from_env(
        point,
        "OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_POINT",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_TRIGGER",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_MARKER",
        "OMEGAT_TEST_RESOLVE_CANCELLATION_FOLLOWUP_RELEASE",
    )
}

fn resolve_cancellation_lock_checkpoint(
    variable: &str,
    props: &ProjectProperties,
    point: &str,
) -> Result<()> {
    let Some(marker) = std::env::var_os(variable) else {
        return Ok(());
    };
    write_json_atomic(
        &PathBuf::from(marker),
        &serde_json::json!({
            "point": point,
            "project_root": normalized(&props.root),
            "sidecar_process_id": std::process::id(),
        }),
    )
    .map_err(|error| TeamError::Command(format!("resolve cancellation lock checkpoint: {error}")))
}

fn product_owner_claim_checkpoint(
    props: &ProjectProperties,
    app_instance: &str,
    process_id: u32,
    generation: u64,
) -> Result<()> {
    let Some(marker) = std::env::var_os("OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_MARKER") else {
        return Ok(());
    };
    let Some(release) = std::env::var_os("OMEGAT_TEST_HOLD_AFTER_PRODUCT_OWNER_CLAIM_RELEASE")
    else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(TeamError::Io(error)),
    };
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({
            "project_root": normalized(&props.root),
            "app_instance": app_instance,
            "process_id": process_id,
            "generation": generation,
        }),
    )
    .map_err(|error| TeamError::Command(format!("renderer owner checkpoint: {error}")))?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    sync_parent(&marker)?;
    let release = PathBuf::from(release);
    while !release.is_file() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

fn compact_terminal_product_transactions(props: &ProjectProperties) -> Result<()> {
    let mut journal = load_product_journal(props)?;
    let terminal = journal
        .batches
        .iter()
        .filter(|transaction| !transaction.0.status.is_recoverable())
        .cloned()
        .collect::<Vec<_>>();
    if terminal.is_empty() {
        if journal.batches.is_empty() {
            let active = transaction_dir(props).join("active.json");
            remove_path(&active)?;
            remove_path(&transaction_dir(props).join(".active.previous.json"))?;
            sync_parent(&active)?;
        }
        return Ok(());
    }
    archive_terminal_product_transactions(props, &terminal)?;
    product_compaction_checkpoint("after_archive_fsync")?;
    if std::env::var("OMEGAT_TEST_ABORT_PRODUCT_COMPACTION_AFTER_ARCHIVE").as_deref() == Ok("1") {
        std::process::abort();
    }
    journal
        .batches
        .retain(|transaction| transaction.0.status.is_recoverable());
    for transaction in &terminal {
        if !transaction.snapshot.as_os_str().is_empty() {
            remove_path(&transaction.snapshot)?;
        }
    }
    // Always publish the compacted v2 queue first, including an empty queue.
    // The atomic writer fsyncs the replacement and parent directory, so a
    // process death at the checkpoint cannot revive an archived terminal row.
    write_product_journal(props, &journal)?;
    product_compaction_checkpoint("after_queue_rename")?;
    if std::env::var("OMEGAT_TEST_ABORT_PRODUCT_COMPACTION_AFTER_QUEUE_RENAME").as_deref()
        == Ok("1")
    {
        std::process::abort();
    }
    if journal.batches.is_empty() {
        let active = transaction_dir(props).join("active.json");
        remove_path(&active)?;
        remove_path(&transaction_dir(props).join(".active.previous.json"))?;
        sync_parent(&active)?;
    }
    Ok(())
}

fn renderer_owner_path(props: &ProjectProperties) -> PathBuf {
    transaction_dir(props).join("renderer-owner.json")
}

fn process_is_alive(process_id: u32) -> bool {
    if process_id == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        Path::new("/proc").join(process_id.to_string()).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Other platforms retain the claim until the same app instance
        // reconnects. Linux has the packaged concurrent-owner evidence.
        let _ = process_id;
        true
    }
}

fn transaction_owner_retry_wait_checkpoint(
    props: &ProjectProperties,
    previous_owner_process_id: u32,
) -> Result<()> {
    let Some(marker) = std::env::var_os("OMEGAT_TEST_TRANSACTION_OWNER_RETRY_WAIT_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    let marker = if marker.exists() {
        PathBuf::from(format!(
            "{}.{previous_owner_process_id}",
            marker.to_string_lossy()
        ))
    } else {
        marker
    };
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if marker.exists() {
        return Ok(());
    }
    write_json_atomic(
        &marker,
        &serde_json::json!({
            "project_root": normalized(&props.root),
            "previous_owner_process_id": previous_owner_process_id,
            "waiting_sidecar_process_id": std::process::id(),
        }),
    )
    .map_err(|error| TeamError::Command(format!("owner retry wait checkpoint: {error}")))
}

/// Wait for the currently recorded dispatcher owner to exit.
///
/// Callers use the returned PID as one bounded replacement-election boundary.
/// Any additional retry is an explicit caller decision and observes the newly
/// published claim again rather than spinning against a stale owner.
pub fn wait_for_transaction_dispatch_owner_exit(
    props: &ProjectProperties,
    timeout: Duration,
) -> Result<Option<u32>> {
    wait_for_transaction_dispatch_owner_exit_cancellable(
        props,
        timeout,
        &CancellationToken::default(),
    )
}

/// Cancellable owner-liveness boundary used by the NDJSON replacement
/// dispatcher. Cancelling a waiting contender never changes the durable owner
/// claim or exposes the product head.
pub fn wait_for_transaction_dispatch_owner_exit_cancellable(
    props: &ProjectProperties,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<Option<u32>> {
    let path = renderer_owner_path(props);
    let deadline = Instant::now() + timeout;
    let claim = loop {
        check_cancelled(cancellation)?;
        match acquire_project_transaction_lock(props) {
            Ok(_lock) => {
                if path.is_file() {
                    let claim: RendererOwnerClaim = serde_json::from_slice(&std::fs::read(&path)?)
                        .map_err(|error| {
                            TeamError::Command(format!("renderer owner claim: {error}"))
                        })?;
                    if claim.version != TRANSACTION_ENVELOPE_VERSION
                        || normalized(&claim.project_root) != normalized(&props.root)
                    {
                        return Err(TeamError::Command(format!(
                            "invalid renderer owner claim at {}",
                            path.display()
                        )));
                    }
                    break claim;
                }
            }
            Err(TeamError::Conflict(_)) => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        // A competing claimant can hold operation.lock while the previous
        // dead owner file is still present. Wait until its atomic claim has
        // been published, then observe that stable owner rather than retrying
        // against a stale PID.
        std::thread::sleep(Duration::from_millis(25));
    };
    transaction_owner_retry_wait_checkpoint(props, claim.process_id)?;
    while process_is_alive(claim.process_id) {
        check_cancelled(cancellation)?;
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(Some(claim.process_id))
}

/// Claim the unified product/refresh renderer dispatcher for one live app.
///
/// The claim is persisted separately from either backend journal, so a second
/// sidecar cannot expose a refresh tail around a product head owned by another
/// Electron process. On Linux, a replacement may take over only after the
/// recorded owner process has exited.
pub fn claim_transaction_dispatch(
    props: &ProjectProperties,
    app_instance: &str,
    process_id: u32,
    generation: u64,
) -> Result<()> {
    if app_instance.is_empty() || process_id == 0 || generation == 0 {
        return Err(TeamError::Command(
            "renderer owner claim requires app instance, process id, and generation".into(),
        ));
    }
    let _lock = acquire_project_transaction_lock(props)?;
    recover_pending_cancellation_locked(props)?;
    let path = renderer_owner_path(props);
    if path.is_file() {
        let claim: RendererOwnerClaim = serde_json::from_slice(&std::fs::read(&path)?)
            .map_err(|error| TeamError::Command(format!("renderer owner claim: {error}")))?;
        if claim.version != TRANSACTION_ENVELOPE_VERSION
            || normalized(&claim.project_root) != normalized(&props.root)
        {
            return Err(TeamError::Command(format!(
                "invalid renderer owner claim at {}",
                path.display()
            )));
        }
        if claim.app_instance != app_instance && process_is_alive(claim.process_id) {
            return Err(TeamError::Conflict(format!(
                "transaction dispatcher is owned by live app {} (pid {})",
                claim.app_instance, claim.process_id
            )));
        }
        if claim.app_instance == app_instance
            && claim.process_id == process_id
            && claim.generation == generation
        {
            return Ok(());
        }
    }
    let sequence = OWNER_CLAIM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let claim = RendererOwnerClaim {
        version: TRANSACTION_ENVELOPE_VERSION,
        project_root: normalized(&props.root),
        app_instance: app_instance.to_string(),
        process_id,
        generation,
        claim_id: format!("{}-{process_id}-{sequence}", unix_ms()),
        updated_unix_ms: unix_ms(),
    };
    write_json_atomic(&path, &claim)
        .map_err(|error| TeamError::Command(format!("renderer owner claim: {error}")))
}

fn capture_product_manifest(
    props: &ProjectProperties,
    external_products: &[ExternalProductSnapshot],
) -> Result<TeamProductManifest> {
    let mut files = Vec::new();
    collect_product_tree(&props.root, "project", true, &mut files)?;
    for (index, repo) in props.repositories.iter().enumerate() {
        if repo.repo_type != "file" || is_inplace(props, repo) {
            continue;
        }
        let remote = PathBuf::from(&repo.url);
        let prefix = format!("file-remote/{index}");
        if remote.exists() || remote.is_symlink() {
            collect_product_path(&remote, &prefix, &mut files)?;
        } else {
            files.push(ProductFileReceipt {
                path: prefix,
                kind: "missing".into(),
                bytes: 0,
                sha256: format!("{:x}", Sha256::digest([])),
            });
        }
    }
    for (index, snapshot) in external_products.iter().enumerate() {
        let prefix = format!("external-product/{index}");
        if snapshot.target.exists() || snapshot.target.is_symlink() {
            collect_product_path(&snapshot.target, &prefix, &mut files)?;
        } else {
            files.push(ProductFileReceipt {
                path: prefix,
                kind: "missing".into(),
                bytes: 0,
                sha256: format!("{:x}", Sha256::digest([])),
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let mut repository_versions = Vec::with_capacity(props.repositories.len());
    for repo in &props.repositories {
        repository_versions.push(if repo.repo_type == "git" {
            remote_repository_factory::file_version(props, repo, "")?
        } else {
            None
        });
    }
    let root_git_version = if props.root.join(".git").exists() {
        Some(crate::git2_ops::current_version(&props.root)?)
    } else {
        None
    };
    Ok(TeamProductManifest {
        files,
        repository_versions,
        root_git_version,
    })
}

fn collect_product_path(
    path: &Path,
    prefix: &str,
    receipts: &mut Vec<ProductFileReceipt>,
) -> Result<()> {
    if path.is_dir() && !path.is_symlink() {
        return collect_product_tree(path, prefix, false, receipts);
    }
    let (kind, bytes) = if path.is_symlink() {
        (
            "symlink",
            std::fs::read_link(path)?
                .to_string_lossy()
                .into_owned()
                .into_bytes(),
        )
    } else {
        ("file", std::fs::read(path)?)
    };
    receipts.push(ProductFileReceipt {
        path: prefix.to_string(),
        kind: kind.into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    });
    Ok(())
}

fn collect_product_tree(
    root: &Path,
    prefix: &str,
    exclude_transaction_state: bool,
    receipts: &mut Vec<ProductFileReceipt>,
) -> Result<()> {
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let Ok(relative) = entry.path().strip_prefix(root) else {
                return false;
            };
            if relative.as_os_str().is_empty() {
                return true;
            }
            let mut components = relative.components();
            let first = components
                .next()
                .map(|component| component.as_os_str().to_string_lossy());
            if matches!(first.as_deref(), Some(".git" | ".svn")) {
                return false;
            }
            !(exclude_transaction_state
                && relative.starts_with(Path::new(".repositories").join("transactions")))
        });
    for entry in walker {
        let entry = entry.map_err(|error| {
            TeamError::Command(format!(
                "walk team product manifest {}: {error}",
                root.display()
            ))
        })?;
        let relative = entry.path().strip_prefix(root).map_err(|error| {
            TeamError::Command(format!(
                "team product manifest path {}: {error}",
                entry.path().display()
            ))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let relative = relative.to_string_lossy().replace('\\', "/");
        let path = format!("{prefix}/{relative}");
        let (kind, bytes) = if entry.file_type().is_symlink() {
            (
                "symlink",
                std::fs::read_link(entry.path())?
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes(),
            )
        } else if entry.file_type().is_dir() {
            ("directory", Vec::new())
        } else {
            ("file", std::fs::read(entry.path())?)
        };
        receipts.push(ProductFileReceipt {
            path,
            kind: kind.into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        });
    }
    Ok(())
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn transaction_dir(props: &ProjectProperties) -> PathBuf {
    props.root.join(".repositories").join("transactions")
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() && !path.is_symlink() {
        std::fs::remove_dir_all(path)?;
    } else if path.exists() || path.is_symlink() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn rollback_repositories(
    props: &ProjectProperties,
    snapshot: &SyncSnapshot,
    rollback_versions: &[Option<String>],
    published: &[usize],
    _commit_started: &[usize],
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut restored = Vec::new();
    for &index in published.iter().rev() {
        let repo = &props.repositories[index];
        let rollback = match repo.repo_type.as_str() {
            "git" => rollback_versions[index]
                .as_deref()
                .map_or(Ok(()), |version| {
                    crate::git_remote_repository2::rollback_published(props, repo, version)
                }),
            "file" => snapshot.restore_file_remote(index),
            _ => Ok(()),
        };
        if let Err(error) = rollback {
            failures.push(format!("repository {index}: {error}"));
        } else {
            restored.push(index);
        }
    }
    for index in (0..props.repositories.len()).rev() {
        if restored.contains(&index) {
            continue;
        }
        let repo = &props.repositories[index];
        let rollback = match repo.repo_type.as_str() {
            "git" => rollback_versions[index]
                .as_deref()
                .map_or(Ok(()), |version| {
                    crate::git_remote_repository2::rollback_unpublished(props, repo, version)
                }),
            "file" => snapshot.restore_file_remote(index),
            _ => Ok(()),
        };
        if let Err(error) = rollback {
            failures.push(format!("repository {index}: {error}"));
        }
    }
    failures
}

/// Recover a persisted multi-repository transaction left by a terminated
/// process before allowing another team operation to mutate the project.
pub fn recover_interrupted_sync(props: &ProjectProperties) -> Result<bool> {
    let _lock = acquire_project_transaction_lock(props)?;
    recover_interrupted_sync_locked(props)
}

fn recover_pending_cancellation_locked(props: &ProjectProperties) -> Result<bool> {
    let journal = load_product_journal(props)?;
    let mut cancelling = journal
        .batches
        .into_iter()
        .filter(|transaction| transaction.0.status == TransactionStatus::CancellationPending);
    let Some(transaction) = cancelling.next() else {
        return Ok(false);
    };
    if cancelling.next().is_some() {
        return Err(TeamError::Command(
            "product transaction journal contains multiple pending cancellations".into(),
        ));
    }
    let mut transaction = transaction;
    validate_pending_resolve_cancellation(props, &transaction)?;
    rollback_pending_resolve_cancellation(props, &mut transaction)?;
    persist_terminal_resolve_cancellation(props, &mut transaction, "renderer-cancelled-recovered")?;
    compact_terminal_product_transactions(props)?;
    Ok(true)
}

fn validate_pending_resolve_cancellation(
    props: &ProjectProperties,
    transaction: &SyncTransaction,
) -> Result<()> {
    if transaction.operation != "resolve-conflict"
        || transaction.0.status != TransactionStatus::CancellationPending
        || transaction.0.error_code != Some(REQUEST_CANCELLED_CODE)
    {
        return Err(TeamError::Command(format!(
            "invalid pending cancellation {}",
            transaction.0.batch_id
        )));
    }
    transaction.validate_repository_shape(props)
}

fn rollback_pending_resolve_cancellation(
    props: &ProjectProperties,
    transaction: &mut SyncTransaction,
) -> Result<()> {
    match transaction.phase.as_str() {
        "renderer-cancelling" => {
            let snapshot = SyncSnapshot::open(
                props,
                transaction.snapshot.clone(),
                transaction.prep_existed,
                transaction.file_remotes.clone(),
                transaction.external_products.clone(),
            )?;
            snapshot.restore_project_and_prep_durable(props)?;
            // Persist the completed rollback before any terminal transition.
            // A second cancel or restart that takes over this exact intent can
            // therefore finish it without opening a second rollback pass.
            transaction.phase = "renderer-rollback-durable".into();
            transaction.persist_preserving_dispatch_order(props)
        }
        "renderer-rollback-durable" => Ok(()),
        phase => Err(TeamError::Command(format!(
            "pending cancellation {} has invalid phase {phase}",
            transaction.0.batch_id
        ))),
    }
}

fn persist_terminal_resolve_cancellation(
    props: &ProjectProperties,
    transaction: &mut SyncTransaction,
    phase: &str,
) -> Result<()> {
    transaction.phase = phase.into();
    transaction.0.transition(
        TransactionStatus::RequestCancelled,
        Some(REQUEST_CANCELLED_CODE),
    );
    transaction.persist(props)
}

fn recover_interrupted_sync_locked(props: &ProjectProperties) -> Result<bool> {
    let recovered_cancellation = recover_pending_cancellation_locked(props)?;
    let journal = load_product_journal(props)?;
    // A committed receipt is renderer-owned work. Leave any terminal prefix in
    // place until transaction.receipt.pending has durably claimed a dispatcher;
    // compaction then runs under that same project lock and owner claim.
    if !journal
        .batches
        .iter()
        .any(|transaction| transaction.0.status == TransactionStatus::SidecarCommitted)
    {
        compact_terminal_product_transactions(props)?;
    }
    let Some(mut transaction) = SyncTransaction::load_active_operation(props)? else {
        return Ok(recovered_cancellation);
    };
    transaction.validate_repository_shape(props)?;
    if transaction.phase == "capturing" {
        transaction.finish(
            props,
            "recovered-capture",
            TransactionStatus::Cancelled,
            None,
        )?;
        return Ok(true);
    }
    let snapshot = SyncSnapshot::open(
        props,
        transaction.snapshot.clone(),
        transaction.prep_existed,
        transaction.file_remotes.clone(),
        transaction.external_products.clone(),
    )?;
    transaction.phase = "recovering".into();
    for index in transaction.commit_started.clone() {
        if transaction.published.contains(&index) {
            continue;
        }
        let repo = &props.repositories[index];
        let Some(rollback_version) = transaction.rollback_versions[index].as_deref() else {
            continue;
        };
        if repo.repo_type == "git"
            && crate::git_remote_repository2::transaction_commit_was_published(
                props,
                repo,
                rollback_version,
            )?
        {
            transaction.published.push(index);
        }
    }
    transaction.published.sort_unstable();
    transaction.published.dedup();
    transaction.persist(props)?;
    let mut failures = rollback_repositories(
        props,
        &snapshot,
        &transaction.rollback_versions,
        &transaction.published,
        &transaction.commit_started,
    );
    if let Some(version) = transaction.root_git_rollback.as_deref() {
        if let Err(error) = crate::git2_ops::reset_hard(&props.root, version) {
            failures.push(format!("root git: {error}"));
        }
    }
    if let Err(error) = snapshot.restore_project_and_prep_durable(props) {
        failures.push(format!("project: {error}"));
    }
    if failures.is_empty() {
        transaction.finish(props, "recovered", TransactionStatus::Cancelled, None)?;
        return Ok(true);
    }
    transaction.phase = "recovery-failed".into();
    let _ = transaction.persist(props);
    Err(TeamError::Command(format!(
        "team transaction recovery failed: {}",
        failures.join(" | ")
    )))
}

/// Commit a local product mutation under the same snapshot, receipt, and
/// restart-recovery state machine used by multi-repository team writes.
pub fn commit_product_transaction_cancellable<T>(
    props: &ProjectProperties,
    operation: &str,
    cancellation: &CancellationToken,
    checkpoint: &'static str,
    generation: u64,
    batch_id: Option<&str>,
    mutation: impl FnOnce(&CancellationToken) -> Result<T>,
) -> Result<T> {
    commit_product_transaction_with_paths_cancellable(
        props,
        operation,
        cancellation,
        checkpoint,
        generation,
        batch_id,
        &[],
        mutation,
    )
}

/// Commit a local mutation whose durable product includes paths outside the
/// project root. External destinations are snapshotted before mutation and
/// restored after cancellation, publish failure, or pre-receipt process death.
pub fn commit_product_transaction_with_paths_cancellable<T>(
    props: &ProjectProperties,
    operation: &str,
    cancellation: &CancellationToken,
    checkpoint: &'static str,
    generation: u64,
    batch_id: Option<&str>,
    external_products: &[PathBuf],
    mutation: impl FnOnce(&CancellationToken) -> Result<T>,
) -> Result<T> {
    check_cancelled(cancellation)?;
    let _lock = acquire_project_transaction_lock(props)?;
    check_cancelled(cancellation)?;
    recover_interrupted_sync_locked(props)?;
    check_cancelled(cancellation)?;
    let (mut journal, snapshot) = SyncTransaction::begin_local_cancellable(
        props,
        operation,
        cancellation,
        checkpoint,
        generation,
        batch_id,
        external_products,
    )?;
    journal.phase = "mutating".into();
    journal.persist(props)?;
    let result = mutation(cancellation);
    match result {
        Ok(value) => {
            if let Err(error) = check_cancelled(cancellation) {
                snapshot.restore_project_and_prep_durable(props)?;
                journal.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
            product_transaction_checkpoint(operation, "before_atomic_publish")?;
            let await_renderer_ack = generation != 0 && batch_id.is_some();
            if let Err(error) =
                journal.publish_product_commit(props, "committed", await_renderer_ack)
            {
                snapshot.restore_project_and_prep_durable(props)?;
                journal.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
            product_transaction_checkpoint(operation, "after_atomic_publish")?;
            if !await_renderer_ack {
                journal.cleanup(props)?;
            }
            Ok(value)
        }
        Err(error) => {
            if let Err(rollback_error) = snapshot.restore_project_and_prep_durable(props) {
                journal.phase = "rollback-failed".into();
                let _ = journal.persist(props);
                return Err(TeamError::Command(format!(
                    "{error}; project rollback failed: {rollback_error}"
                )));
            }
            journal.finish_for_error(props, "rolled-back", &error)?;
            Err(error)
        }
    }
}

fn product_transaction_checkpoint(operation: &str, point: &str) -> Result<()> {
    if std::env::var("OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION").as_deref() != Ok(operation)
        || std::env::var("OMEGAT_TEST_PRODUCT_TRANSACTION_POINT").as_deref() != Ok(point)
    {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    writeln!(file, "{operation}:{point}")?;
    file.sync_all()?;
    if let Some(parent) = marker.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

pub fn sync(props: &ProjectProperties) -> Result<SyncReport> {
    sync_cancellable(props, &CancellationToken::default())
}

pub fn sync_cancellable(
    props: &ProjectProperties,
    cancellation: &CancellationToken,
) -> Result<SyncReport> {
    sync_cancellable_scoped(props, cancellation, 0, None)
}

pub fn sync_cancellable_scoped(
    props: &ProjectProperties,
    cancellation: &CancellationToken,
    generation: u64,
    batch_id: Option<&str>,
) -> Result<SyncReport> {
    check_cancelled(cancellation)?;
    let _lock = acquire_project_transaction_lock(props)?;
    check_cancelled(cancellation)?;
    let recovered = recover_interrupted_sync_locked(props)?;
    check_cancelled(cancellation)?;
    if !team_enabled() {
        return Ok(SyncReport {
            action: "skipped".into(),
            message: if recovered {
                "recovered interrupted transaction; --no-team".into()
            } else {
                "--no-team".into()
            },
            conflicts: vec![],
        });
    }
    let mut report = SyncReport {
        action: "sync".into(),
        message: String::new(),
        conflicts: vec![],
    };
    if props.repositories.is_empty() {
        report.action = "local".into();
        report.message = "no repositories".into();
        return Ok(report);
    }

    let (mut journal, snapshot) = SyncTransaction::begin(props, "sync", generation, batch_id)?;
    let await_renderer_ack = generation != 0 && batch_id.is_some();
    let mut observed = vec![None; props.repositories.len()];
    let mut rollback_versions = vec![None; props.repositories.len()];
    let mut published = Vec::new();
    let mut commit_started = Vec::new();
    let mut pending_conflicts = None;
    let transaction = (|| -> Result<()> {
        check_cancelled(cancellation)?;
        journal.phase = "preparing".into();
        journal.persist(props)?;
        std::fs::create_dir_all(prep_dir(props))?;
        for repo in &props.repositories {
            check_cancelled(cancellation)?;
            remote_repository_factory::prepare(props, repo)?;
            check_cancelled(cancellation)?;
        }
        let mut deleted = Vec::with_capacity(props.repositories.len());
        for (index, repo) in props.repositories.iter().enumerate() {
            check_cancelled(cancellation)?;
            if repo.repo_type == "git" {
                rollback_versions[index] =
                    remote_repository_factory::file_version(props, repo, "")?;
            }
            let mappings = effective_mappings(repo);
            let version_path = mappings
                .first()
                .map(|mapping| mapping.repository.trim_matches(['/', '\\']))
                .unwrap_or("");
            observed[index] = remote_repository_factory::file_version(props, repo, version_path)?;
            deleted.push(remote_repository_factory::recently_deleted_files(
                props, repo,
            )?);
            check_cancelled(cancellation)?;
        }
        journal.rollback_versions.clone_from(&rollback_versions);
        journal.phase = "prepared".into();
        journal.persist(props)?;
        journal.phase = "copying-remote".into();
        journal.persist(props)?;
        for (repo, deleted) in props.repositories.iter().zip(&deleted) {
            check_cancelled(cancellation)?;
            copy_mapped_cancellable(props, repo, CopyDir::RepoToProject, cancellation)?;
            propagate_deleted(props, repo, deleted)?;
            check_cancelled(cancellation)?;
        }
        journal.phase = "rebasing".into();
        journal.persist(props)?;
        check_cancelled(cancellation)?;
        let conflicts = rebase_all(props)?;
        check_cancelled(cancellation)?;
        if !conflicts.is_empty() {
            pending_conflicts = Some(conflicts.clone());
            return Err(TeamError::Conflict(
                conflicts
                    .iter()
                    .map(|c| format!("{}:{}", c.kind, c.source))
                    .collect::<Vec<_>>()
                    .join(" | "),
            ));
        }
        journal.phase = "staging".into();
        journal.persist(props)?;
        for repo in &props.repositories {
            check_cancelled(cancellation)?;
            copy_mapped_cancellable(props, repo, CopyDir::ProjectToRepo, cancellation)?;
            check_cancelled(cancellation)?;
        }
        journal.phase = "publishing".into();
        journal.persist(props)?;
        for index in 0..props.repositories.len() {
            check_cancelled(cancellation)?;
            commit_started.push(index);
            journal.commit_started.clone_from(&commit_started);
            journal.persist(props)?;
            commit_repository(props, index, &[observed[index].clone()], "OmegaT team sync")?;
            #[cfg(test)]
            if CRASH_AFTER_PUBLISH_REPOSITORY
                .compare_exchange(index, usize::MAX, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                std::process::abort();
            }
            published.push(index);
            journal.published.clone_from(&published);
            journal.persist(props)?;
            check_cancelled(cancellation)?;
        }
        journal.phase = "saving-bases".into();
        journal.persist(props)?;
        check_cancelled(cancellation)?;
        save_bases(props)?;
        save_conflicts(props, &[])?;
        clear_resolved(props);
        check_cancelled(cancellation)?;
        Ok(())
    })();

    if let Err(error) = transaction {
        let mut rollback_failures = rollback_repositories(
            props,
            &snapshot,
            &rollback_versions,
            &published,
            &commit_started,
        );
        if let Err(rollback_error) = snapshot.restore_project_and_prep_durable(props) {
            rollback_failures.push(format!("project: {rollback_error}"));
        }
        if let Some(conflicts) = pending_conflicts {
            if let Err(conflict_error) = save_conflicts(props, &conflicts) {
                rollback_failures.push(format!("conflicts: {conflict_error}"));
            }
        }
        if rollback_failures.is_empty() {
            journal.finish_for_error(props, "rolled-back", &error)?;
            return Err(error);
        }
        journal.phase = "rollback-failed".into();
        let _ = journal.persist(props);
        return Err(TeamError::Command(format!(
            "{error}; rollback failed: {}",
            rollback_failures.join(" | ")
        )));
    }

    let publish_result = product_transaction_checkpoint("team.sync", "before_atomic_publish")
        .and_then(|_| journal.publish_product_commit(props, "committed", await_renderer_ack));
    if let Err(error) = publish_result {
        let mut rollback_failures = rollback_repositories(
            props,
            &snapshot,
            &rollback_versions,
            &published,
            &commit_started,
        );
        if let Err(rollback_error) = snapshot.restore_project_and_prep_durable(props) {
            rollback_failures.push(format!("project: {rollback_error}"));
        }
        if rollback_failures.is_empty() {
            journal.finish_for_error(props, "rolled-back", &error)?;
            return Err(error);
        }
        journal.phase = "rollback-failed".into();
        let _ = journal.persist(props);
        return Err(TeamError::Command(format!(
            "{error}; rollback failed: {}",
            rollback_failures.join(" | ")
        )));
    }
    product_transaction_checkpoint("team.sync", "after_atomic_publish")?;
    if !await_renderer_ack {
        journal.cleanup(props)?;
    }
    for repo in &props.repositories {
        report
            .message
            .push_str(&format!("synced {}; ", repo.repo_type));
    }
    if recovered {
        report
            .message
            .push_str("recovered interrupted transaction; ");
    }
    Ok(report)
}

pub fn commit_project_files(props: &ProjectProperties, which: &str) -> Result<SyncReport> {
    commit_project_files_cancellable(props, which, &CancellationToken::default())
}

pub fn commit_project_files_cancellable(
    props: &ProjectProperties,
    which: &str,
    cancellation: &CancellationToken,
) -> Result<SyncReport> {
    commit_project_files_cancellable_scoped(props, which, cancellation, 0, None)
}

pub fn commit_project_files_cancellable_scoped(
    props: &ProjectProperties,
    which: &str,
    cancellation: &CancellationToken,
    generation: u64,
    batch_id: Option<&str>,
) -> Result<SyncReport> {
    check_cancelled(cancellation)?;
    let _lock = acquire_project_transaction_lock(props)?;
    check_cancelled(cancellation)?;
    let recovered = recover_interrupted_sync_locked(props)?;
    check_cancelled(cancellation)?;
    let label = match which {
        "source" | "target" => which,
        _ => {
            return Err(TeamError::Command(format!(
                "commit which must be source or target, got {which}"
            )))
        }
    };
    let dir = if label == "source" {
        &props.source_dir
    } else {
        &props.target_dir
    };
    if !dir.exists() {
        return Err(TeamError::Command(format!("{label} directory missing")));
    }
    let root_git = props.root.join(".git").exists();
    if !props.repositories.is_empty() || root_git {
        let await_renderer_ack = generation != 0 && batch_id.is_some();
        let (mut journal, snapshot) =
            SyncTransaction::begin(props, &format!("commit-{label}"), generation, batch_id)?;
        let mut rollback_versions = vec![None; props.repositories.len()];
        let mut root_git_rollback = None;
        let mut published = Vec::new();
        let mut commit_started = Vec::new();
        let transaction = (|| -> Result<()> {
            check_cancelled(cancellation)?;
            journal.phase = "observing".into();
            journal.persist(props)?;
            for (index, repo) in props.repositories.iter().enumerate() {
                check_cancelled(cancellation)?;
                if repo.repo_type == "git" {
                    rollback_versions[index] =
                        remote_repository_factory::file_version(props, repo, "")?;
                }
            }
            if root_git {
                root_git_rollback = Some(crate::git2_ops::current_version(&props.root)?);
            }
            journal.rollback_versions.clone_from(&rollback_versions);
            journal.root_git_rollback.clone_from(&root_git_rollback);
            journal.phase = "staging".into();
            journal.persist(props)?;
            for repo in &props.repositories {
                check_cancelled(cancellation)?;
                copy_mapped_cancellable(props, repo, CopyDir::ProjectToRepo, cancellation)?;
                check_cancelled(cancellation)?;
            }
            if root_git {
                check_cancelled(cancellation)?;
            }
            journal.phase = "publishing".into();
            journal.persist(props)?;
            for index in 0..props.repositories.len() {
                check_cancelled(cancellation)?;
                commit_started.push(index);
                journal.commit_started.clone_from(&commit_started);
                journal.persist(props)?;
                commit_repository(
                    props,
                    index,
                    &[rollback_versions[index].clone()],
                    &format!("OmegaT commit {label} files"),
                )?;
                published.push(index);
                journal.published.clone_from(&published);
                journal.persist(props)?;
                check_cancelled(cancellation)?;
            }
            if root_git {
                crate::git2_ops::commit_project_tree(
                    &props.root,
                    &format!("OmegaT commit {label} files"),
                )?;
                check_cancelled(cancellation)?;
            }
            Ok(())
        })();
        if let Err(error) = transaction {
            let mut rollback_failures = rollback_repositories(
                props,
                &snapshot,
                &rollback_versions,
                &published,
                &commit_started,
            );
            if let Some(version) = root_git_rollback.as_deref() {
                if let Err(rollback_error) = crate::git2_ops::reset_hard(&props.root, version) {
                    rollback_failures.push(format!("root git: {rollback_error}"));
                }
            }
            if let Err(rollback_error) = snapshot.restore_project_and_prep_durable(props) {
                rollback_failures.push(format!("project: {rollback_error}"));
            }
            if rollback_failures.is_empty() {
                journal.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
            journal.phase = "rollback-failed".into();
            let _ = journal.persist(props);
            return Err(TeamError::Command(format!(
                "{error}; rollback failed: {}",
                rollback_failures.join(" | ")
            )));
        }
        let publish_result = product_transaction_checkpoint("team.commit", "before_atomic_publish")
            .and_then(|_| journal.publish_product_commit(props, "committed", await_renderer_ack));
        if let Err(error) = publish_result {
            let mut rollback_failures = rollback_repositories(
                props,
                &snapshot,
                &rollback_versions,
                &published,
                &commit_started,
            );
            if let Some(version) = root_git_rollback.as_deref() {
                if let Err(rollback_error) = crate::git2_ops::reset_hard(&props.root, version) {
                    rollback_failures.push(format!("root git: {rollback_error}"));
                }
            }
            if let Err(rollback_error) = snapshot.restore_project_and_prep_durable(props) {
                rollback_failures.push(format!("project: {rollback_error}"));
            }
            if rollback_failures.is_empty() {
                journal.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
            journal.phase = "rollback-failed".into();
            let _ = journal.persist(props);
            return Err(TeamError::Command(format!(
                "{error}; rollback failed: {}",
                rollback_failures.join(" | ")
            )));
        }
        product_transaction_checkpoint("team.commit", "after_atomic_publish")?;
        if !await_renderer_ack {
            journal.cleanup(props)?;
        }
    }
    Ok(SyncReport {
        action: format!("commit-{label}"),
        message: format!(
            "committed {label} under {}{}",
            dir.display(),
            if recovered {
                "; recovered interrupted transaction"
            } else {
                ""
            }
        ),
        conflicts: vec![],
    })
}

fn refresh_result_items(result: &serde_json::Value) -> u64 {
    result
        .get("entry_list")
        .and_then(serde_json::Value::as_array)
        .map_or(0, |entries| entries.len() as u64)
}

fn refresh_transaction(
    props: &ProjectProperties,
    envelope: TransactionEnvelope<TransactionRendererPayload>,
) -> Result<SyncTransaction> {
    envelope
        .validate_for_root(&props.root)
        .map_err(|error| TeamError::Command(format!("refresh transaction: {error}")))?;
    let operation = envelope.payload.operation.clone();
    if operation != "project.external-refresh" {
        return Err(TeamError::Command(format!(
            "unsupported refresh operation {operation}"
        )));
    }
    let transaction = SyncTransaction(TransactionEnvelope {
        version: envelope.version,
        project_root: envelope.project_root,
        generation: envelope.generation,
        batch_id: envelope.batch_id,
        status: envelope.status,
        error_code: envelope.error_code,
        updated_unix_ms: envelope.updated_unix_ms,
        payload: SyncTransactionPayload {
            operation,
            phase: match envelope.status {
                TransactionStatus::Pending => "refresh-pending",
                TransactionStatus::SidecarCommitted => "refresh-committed",
                TransactionStatus::Completed => "renderer-acknowledged",
                TransactionStatus::RequestCancelled => "refresh-request-cancelled",
                TransactionStatus::Cancelled => "refresh-cancelled",
                TransactionStatus::CancellationPending => "refresh-cancelling",
            }
            .into(),
            snapshot: PathBuf::new(),
            prep_existed: false,
            file_remotes: Vec::new(),
            repository_count: 0,
            rollback_versions: Vec::new(),
            commit_started: Vec::new(),
            published: Vec::new(),
            product_manifest: None,
            root_git_rollback: None,
            refresh: Some(envelope.payload),
            external_products: Vec::new(),
        },
        commit: envelope.commit,
    });
    transaction.validate_loaded(props)?;
    Ok(transaction)
}

fn refresh_envelope(transaction: &SyncTransaction) -> Result<TransactionRendererReceipt> {
    if !transaction.is_refresh() {
        return Err(TeamError::Command(format!(
            "transaction {} is not an external refresh",
            transaction.0.batch_id
        )));
    }
    transaction.renderer_receipt()
}

/// Atomically merge the former external-refresh journal into the shared FIFO.
///
/// The caller removes legacy files only after this function succeeds. Repeating
/// migration after a process death is idempotent by batch id and exact payload.
pub fn migrate_refresh_transactions(
    props: &ProjectProperties,
    active: Vec<TransactionEnvelope<TransactionRendererPayload>>,
    history: Vec<TransactionEnvelope<TransactionRendererPayload>>,
) -> Result<()> {
    if active.is_empty() && history.is_empty() {
        return Ok(());
    }
    let _lock = acquire_project_transaction_lock(props)?;
    recover_interrupted_sync_locked(props)?;
    let mut journal = load_product_journal(props)?;
    for envelope in active {
        let transaction = refresh_transaction(props, envelope)?;
        if let Some(existing) = journal
            .batches
            .iter()
            .find(|existing| existing.0.batch_id == transaction.0.batch_id)
        {
            if serde_json::to_value(existing).ok() != serde_json::to_value(&transaction).ok() {
                return Err(TeamError::Command(format!(
                    "refresh migration batch {} conflicts with the shared journal",
                    transaction.0.batch_id
                )));
            }
            continue;
        }
        journal.batches.push(transaction);
    }
    journal.batches.sort_by(|left, right| {
        left.0
            .updated_unix_ms
            .cmp(&right.0.updated_unix_ms)
            .then_with(|| left.0.batch_id.cmp(&right.0.batch_id))
            .then_with(|| left.operation.cmp(&right.operation))
    });
    write_product_journal(props, &journal)?;

    let terminal = history
        .into_iter()
        .map(|envelope| refresh_transaction(props, envelope))
        .collect::<Result<Vec<_>>>()?;
    if !terminal.is_empty() {
        archive_terminal_product_transactions(props, &terminal)?;
    }
    Ok(())
}

/// Append or coalesce one pending refresh in the shared durable FIFO.
pub fn enqueue_refresh_transaction(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    paths: Vec<String>,
    fingerprints: BTreeMap<String, Option<String>>,
    sources: Vec<String>,
) -> Result<TransactionRendererReceipt> {
    if generation == 0
        || batch_id.is_empty()
        || paths.is_empty()
        || sources.is_empty()
        || !sources
            .iter()
            .all(|source| matches!(source.as_str(), "native" | "sidecar"))
    {
        return Err(TeamError::Command(
            "refresh enqueue requires generation, batch id, paths, and native/sidecar sources"
                .into(),
        ));
    }
    let _lock = acquire_project_transaction_lock(props)?;
    recover_interrupted_sync_locked(props)?;
    let journal = load_product_journal(props)?;
    if let Some(existing) = journal.batches.into_iter().find(|transaction| {
        transaction.is_refresh()
            && transaction.0.status == TransactionStatus::Pending
            && transaction
                .refresh
                .as_ref()
                .is_some_and(|refresh| refresh.fingerprints == fingerprints)
    }) {
        let mut existing = existing;
        let refresh = existing
            .refresh
            .as_mut()
            .expect("refresh transaction has refresh payload");
        for source in sources {
            if !refresh.sources.contains(&source) {
                refresh.sources.push(source);
            }
        }
        refresh.sources.sort();
        existing.persist_preserving_dispatch_order(props)?;
        return refresh_envelope(&existing);
    }

    let envelope = TransactionEnvelope::pending(
        &props.root,
        generation,
        batch_id,
        TransactionRendererPayload {
            operation: "project.external-refresh".into(),
            paths,
            fingerprints,
            sources,
            committed_result: None,
        },
    );
    let mut transaction = refresh_transaction(props, envelope)?;
    transaction.persist_preserving_dispatch_order(props)?;
    refresh_envelope(&transaction)
}

/// Publish the exact refresh result and receipt in the shared queue rename.
pub fn checkpoint_refresh_transaction(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    committed_result: &serde_json::Value,
) -> Result<TransactionRendererReceipt> {
    let _lock = acquire_project_transaction_lock(props)?;
    let Some(mut transaction) = SyncTransaction::load_dispatch_head(props)? else {
        return Err(TeamError::Conflict(format!(
            "refresh batch {batch_id} is no longer pending"
        )));
    };
    if transaction.0.batch_id != batch_id
        || transaction.0.generation != generation
        || !transaction.is_refresh()
    {
        return Err(TeamError::Conflict(format!(
            "shared FIFO head is {}, not refresh {batch_id}",
            transaction.0.batch_id
        )));
    }
    if transaction.0.status == TransactionStatus::SidecarCommitted {
        return refresh_envelope(&transaction);
    }
    let dispatch_unix_ms = transaction.0.updated_unix_ms;
    transaction
        .refresh
        .as_mut()
        .expect("refresh transaction has refresh payload")
        .committed_result = Some(committed_result.clone());
    transaction
        .0
        .commit_product(
            TransactionStatus::SidecarCommitted,
            committed_result,
            refresh_result_items(committed_result),
        )
        .map_err(|error| TeamError::Command(format!("refresh transaction: {error}")))?;
    transaction.0.updated_unix_ms = dispatch_unix_ms;
    transaction.phase = "refresh-committed".into();
    transaction.persist_preserving_dispatch_order(props)?;
    refresh_envelope(&transaction)
}

/// Complete a protocol-cancelled refresh without exposing it for replay.
pub fn cancel_refresh_transaction(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
) -> Result<()> {
    acknowledge_transaction_receipt_outcome(
        props,
        generation,
        batch_id,
        "project.external-refresh",
        "cancelled",
    )
    .map(|_| ())
}

/// Drop only refresh rows at a same-process project-generation boundary.
pub fn discard_refresh_transactions(props: &ProjectProperties) -> Result<()> {
    let _lock = acquire_project_transaction_lock(props)?;
    let mut journal = load_product_journal(props)?;
    let mut changed = false;
    for transaction in &mut journal.batches {
        if !transaction.is_refresh() || !transaction.0.status.is_recoverable() {
            continue;
        }
        transaction.phase = "refresh-discarded".into();
        match transaction.0.status {
            TransactionStatus::Pending => {
                transaction.0.transition(TransactionStatus::Cancelled, None)
            }
            TransactionStatus::SidecarCommitted => {
                transaction.0.transition(TransactionStatus::Completed, None)
            }
            _ => continue,
        }
        changed = true;
    }
    if changed {
        write_product_journal(props, &journal)?;
    }
    compact_terminal_product_transactions(props)
}

fn acknowledged_receipt_in_history(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    operation: &str,
) -> Result<bool> {
    let path = transaction_dir(props).join("history.ndjson");
    let Ok(history) = std::fs::read_to_string(&path) else {
        return Ok(false);
    };
    for line in history.lines().rev().filter(|line| !line.trim().is_empty()) {
        let transaction: SyncTransaction = serde_json::from_str(line)
            .map_err(|error| TeamError::Command(format!("team transaction history: {error}")))?;
        if transaction.0.batch_id == batch_id {
            return Ok(transaction.0.generation == generation
                && !transaction.0.status.is_recoverable()
                && transaction.phase == "renderer-acknowledged"
                && transaction.operation == operation);
        }
    }
    Ok(false)
}

/// Return the FIFO head committed product receipt that still requires renderer ack.
///
/// A replacement renderer adopts the receipt under its current project
/// generation. Pending pre-commit transactions are recovered separately and
/// are never exposed as committed work.
pub fn pending_transaction_receipt(
    props: &ProjectProperties,
    generation: u64,
) -> Result<Option<TransactionRendererReceipt>> {
    pending_transaction_receipt_for_owner(
        props,
        generation,
        &format!("direct-{}", std::process::id()),
        std::process::id(),
    )
}

pub fn pending_transaction_receipt_for_owner(
    props: &ProjectProperties,
    generation: u64,
    app_instance: &str,
    process_id: u32,
) -> Result<Option<TransactionRendererReceipt>> {
    if generation == 0 {
        return Err(TeamError::Command(
            "renderer receipt generation must be non-zero".into(),
        ));
    }
    claim_transaction_dispatch(props, app_instance, process_id, generation)?;
    // This durable boundary deliberately sits after atomic owner publication
    // and before queue compaction/head lookup. A killed claimant therefore
    // cannot leak a half-returned envelope, and a later process must run the
    // same owner election before it can observe the FIFO head.
    product_owner_claim_checkpoint(props, app_instance, process_id, generation)?;
    let _lock = acquire_project_transaction_lock(props)?;
    compact_terminal_product_transactions(props)?;
    let Some(mut transaction) = SyncTransaction::load_dispatch_head(props)? else {
        return Ok(None);
    };
    transaction.validate_repository_shape(props)?;
    if transaction.0.generation != generation {
        // Renderer adoption changes ownership, not durable FIFO order.
        transaction.0.generation = generation;
        transaction.persist_preserving_dispatch_order(props)?;
    }
    transaction.renderer_receipt().map(Some)
}

/// Inspect a committed renderer receipt without adopting it into a new
/// renderer generation.
///
/// Recovery discovery uses this read-only view to identify a project-close
/// receipt while no project is open. Generation adoption remains the
/// responsibility of [`pending_transaction_receipt`], after a caller has
/// selected the exact project root.
pub fn peek_transaction_receipt(
    props: &ProjectProperties,
) -> Result<Option<TransactionRendererReceipt>> {
    let _lock = acquire_project_transaction_lock(props)?;
    let Some(transaction) = SyncTransaction::load_product_receipt_head(props)? else {
        return Ok(None);
    };
    transaction.validate_repository_shape(props)?;
    transaction.renderer_receipt().map(Some)
}

/// Return one exact committed receipt for the product RPC that created it.
///
/// This does not claim or reorder the dispatcher. A direct reply may describe
/// a FIFO tail while `transaction.receipt.pending` continues to expose only
/// the older durable head.
pub fn transaction_receipt(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
) -> Result<Option<TransactionRendererReceipt>> {
    if generation == 0 || batch_id.is_empty() {
        return Err(TeamError::Command(
            "transaction receipt requires generation and batch id".into(),
        ));
    }
    let _lock = acquire_project_transaction_lock(props)?;
    let Some(transaction) = SyncTransaction::load_receipt(props, batch_id)? else {
        return Ok(None);
    };
    transaction.validate_repository_shape(props)?;
    transaction.renderer_receipt().map(Some)
}

/// Idempotently acknowledge a product receipt after renderer publication.
///
/// Only this transition removes the exact journal row and its rollback snapshot. If
/// the acknowledgement response is lost, repeating the same RPC consults the
/// durable completed history and performs no product write or compensation.
pub fn acknowledge_transaction_receipt(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    operation: &str,
) -> Result<TransactionRendererAck> {
    acknowledge_transaction_receipt_outcome(props, generation, batch_id, operation, "succeeded")
}

/// Idempotently acknowledge the global product/refresh FIFO head.
///
/// Pending refresh rows may be acknowledged as coalesced without executing
/// their product read. Every committed row is completed only after renderer
/// publication. Product writes accept only the succeeded outcome.
pub fn acknowledge_transaction_receipt_outcome(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    operation: &str,
    outcome: &str,
) -> Result<TransactionRendererAck> {
    if generation == 0 || batch_id.is_empty() || operation.is_empty() {
        return Err(TeamError::Command(
            "renderer acknowledgement requires generation, batch id, and operation".into(),
        ));
    }
    if !matches!(outcome, "succeeded" | "cancelled" | "coalesced") {
        return Err(TeamError::Command(format!(
            "unsupported renderer acknowledgement outcome {outcome}"
        )));
    }
    let _lock = acquire_project_transaction_lock(props)?;
    if let Some(mut transaction) = SyncTransaction::load_dispatch_head(props)? {
        transaction.validate_repository_shape(props)?;
        if transaction.0.batch_id != batch_id {
            return Err(TeamError::Conflict(format!(
                "renderer receipt is {}, not {batch_id}",
                transaction.0.batch_id
            )));
        }
        if transaction.0.generation != generation {
            return Err(TeamError::Conflict(format!(
                "renderer receipt {} belongs to generation {}, not {generation}",
                transaction.0.batch_id, transaction.0.generation
            )));
        }
        if transaction.operation != operation {
            return Err(TeamError::Conflict(format!(
                "renderer receipt {batch_id} is for {}, not {operation}",
                transaction.operation
            )));
        }
        if !transaction.is_refresh() && outcome != "succeeded" {
            return Err(TeamError::Conflict(format!(
                "product receipt {batch_id} cannot be acknowledged as {outcome}"
            )));
        }
        if transaction.0.status == TransactionStatus::Pending
            && !(transaction.is_refresh() && matches!(outcome, "cancelled" | "coalesced"))
        {
            return Err(TeamError::Conflict(format!(
                "pending refresh {batch_id} requires cancelled or coalesced outcome"
            )));
        }
        if !matches!(
            transaction.0.status,
            TransactionStatus::Pending | TransactionStatus::SidecarCommitted
        ) {
            return Err(TeamError::Conflict(format!(
                "transaction {batch_id} is not awaiting renderer acknowledgement"
            )));
        }
        transaction.phase = "renderer-acknowledged".into();
        if outcome == "cancelled" {
            transaction.0.transition(
                TransactionStatus::RequestCancelled,
                Some(REQUEST_CANCELLED_CODE),
            );
        } else {
            transaction.0.transition(TransactionStatus::Completed, None);
        }
        transaction.persist(props)?;
        transaction.cleanup(props)?;
        return Ok(TransactionRendererAck {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(&props.root),
            generation,
            batch_id: batch_id.to_string(),
            acknowledged: true,
            already_acknowledged: false,
        });
    }
    if acknowledged_receipt_in_history(props, generation, batch_id, operation)? {
        return Ok(TransactionRendererAck {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(&props.root),
            generation,
            batch_id: batch_id.to_string(),
            acknowledged: true,
            already_acknowledged: true,
        });
    }
    Err(TeamError::Conflict(format!(
        "unknown renderer receipt {batch_id}"
    )))
}

/// Roll back a committed-but-undelivered conflict resolution after the user
/// cancels at the dispatcher boundary.
///
/// Only `resolve-conflict` is eligible: unlike sync/commit, it has not
/// published a repository mutation, and its retained local snapshot can be
/// restored atomically before the durable receipt becomes request-cancelled.
pub fn cancel_transaction_receipt(
    props: &ProjectProperties,
    generation: u64,
    batch_id: &str,
    operation: &str,
) -> Result<()> {
    if generation == 0 || batch_id.is_empty() || operation != "resolve-conflict" {
        return Err(TeamError::Command(
            "only a scoped resolve-conflict receipt can be cancelled".into(),
        ));
    }
    // Concurrent cancellation acknowledgements target the same durable
    // idempotency key. Unlike unrelated team operations, the loser must wait
    // for the current cancellation owner (or its OS-released lock after
    // process death), then observe the sole terminal decision as -32800.
    let lock = wait_for_project_transaction_lock(props)?;
    let Some(mut transaction) = load_product_journal(props)?
        .batches
        .into_iter()
        .find(|transaction| transaction.0.batch_id == batch_id)
    else {
        let path = transaction_dir(props).join("history.ndjson");
        if let Ok(history) = std::fs::read_to_string(path) {
            for line in history.lines().rev().filter(|line| !line.trim().is_empty()) {
                let archived: SyncTransaction = serde_json::from_str(line).map_err(|error| {
                    TeamError::Command(format!("team transaction history: {error}"))
                })?;
                if archived.0.batch_id == batch_id {
                    if archived.0.generation == generation
                        && archived.operation == operation
                        && archived.0.status == TransactionStatus::RequestCancelled
                        && archived.0.error_code == Some(REQUEST_CANCELLED_CODE)
                    {
                        return Ok(());
                    }
                    break;
                }
            }
        }
        return Err(TeamError::Conflict(format!(
            "unknown renderer receipt {batch_id}"
        )));
    };
    transaction.validate_repository_shape(props)?;
    if transaction.0.batch_id != batch_id
        || transaction.0.generation != generation
        || transaction.operation != operation
    {
        return Err(TeamError::Conflict(format!(
            "renderer receipt does not match cancelled resolve {batch_id}"
        )));
    }
    if transaction.0.status == TransactionStatus::CancellationPending {
        if lock.waited {
            resolve_cancellation_lock_checkpoint(
                "OMEGAT_TEST_RESOLVE_CANCELLATION_TAKEOVER_MARKER",
                props,
                "took-over-pending-cancellation",
            )?;
        }
        validate_pending_resolve_cancellation(props, &transaction)?;
        rollback_pending_resolve_cancellation(props, &mut transaction)?;
        resolve_cancellation_checkpoint("after_rollback_fsync")?;
        persist_terminal_resolve_cancellation(
            props,
            &mut transaction,
            "renderer-cancelled-takeover",
        )?;
        resolve_cancellation_checkpoint("after_terminal_queue_rename")?;
        return compact_terminal_product_transactions(props);
    }
    if transaction.0.status == TransactionStatus::RequestCancelled
        && transaction.0.error_code == Some(REQUEST_CANCELLED_CODE)
    {
        // Idempotent cancellation callers converge through the same durable
        // archive/queue-rename compactor as dispatcher recovery. A waiter that
        // acquired operation.lock only after the terminal publisher died must
        // never rewrite the rollback or terminal decision.
        return compact_terminal_product_transactions(props);
    }
    if transaction.0.status != TransactionStatus::SidecarCommitted {
        return Err(TeamError::Conflict(format!(
            "resolve receipt {batch_id} is not cancellable from {:?}",
            transaction.0.status
        )));
    }
    if !transaction.published.is_empty() || !transaction.commit_started.is_empty() {
        return Err(TeamError::Conflict(format!(
            "resolve receipt {batch_id} published repository work"
        )));
    }
    let committed_manifest = transaction.product_manifest.as_ref().ok_or_else(|| {
        TeamError::Command(format!(
            "resolve receipt {batch_id} has no committed product manifest"
        ))
    })?;
    if &capture_product_manifest(props, &transaction.external_products)? != committed_manifest {
        return Err(TeamError::Conflict(format!(
            "resolve receipt {batch_id} product changed before cancellation"
        )));
    }
    let dispatch_order = transaction.0.updated_unix_ms;
    transaction.phase = "renderer-cancelling".into();
    transaction.0.transition(
        TransactionStatus::CancellationPending,
        Some(REQUEST_CANCELLED_CODE),
    );
    // Cancelling a FIFO tail must not change the durable order of any older
    // receipt. The row becomes undispatchable in the same atomic queue rename.
    transaction.0.updated_unix_ms = dispatch_order;
    transaction.persist_preserving_dispatch_order(props)?;
    resolve_cancellation_checkpoint("after_intent_queue_rename")?;

    rollback_pending_resolve_cancellation(props, &mut transaction)?;
    resolve_cancellation_checkpoint("after_rollback_fsync")?;

    persist_terminal_resolve_cancellation(props, &mut transaction, "renderer-cancelled")?;
    resolve_cancellation_checkpoint("after_terminal_queue_rename")?;
    compact_terminal_product_transactions(props)
}

pub fn get_version(
    props: &ProjectProperties,
    repository_index: usize,
    file: &str,
) -> Result<Option<String>> {
    let repo = props.repositories.get(repository_index).ok_or_else(|| {
        TeamError::Command(format!(
            "repository index {repository_index} is out of range"
        ))
    })?;
    remote_repository_factory::file_version(props, repo, file)
}

pub fn switch_to_version(
    props: &ProjectProperties,
    repository_index: usize,
    version: Option<&str>,
) -> Result<()> {
    let _lock = acquire_project_transaction_lock(props)?;
    let repo = props.repositories.get(repository_index).ok_or_else(|| {
        TeamError::Command(format!(
            "repository index {repository_index} is out of range"
        ))
    })?;
    remote_repository_factory::switch_to_version(props, repo, version)
}

pub fn commit_after_version(
    props: &ProjectProperties,
    repository_index: usize,
    versions: &[Option<String>],
    comment: &str,
) -> Result<Option<String>> {
    let _lock = acquire_project_transaction_lock(props)?;
    let repo = props.repositories.get(repository_index).ok_or_else(|| {
        TeamError::Command(format!(
            "repository index {repository_index} is out of range"
        ))
    })?;
    remote_repository_factory::commit_after_versions(props, repo, versions, comment)
}
