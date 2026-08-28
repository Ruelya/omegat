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
    Ok(ProjectTransactionLock { _file: file })
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

struct SyncSnapshot {
    base: PathBuf,
    project: PathBuf,
    prep: PathBuf,
    prep_existed: bool,
    file_remotes: Vec<FileRemoteSnapshot>,
}

impl SyncSnapshot {
    fn capture(props: &ProjectProperties, base: PathBuf) -> Result<Self> {
        Self::capture_cancellable(props, base, true, &CancellationToken::default(), None)
    }

    fn capture_cancellable(
        props: &ProjectProperties,
        base: PathBuf,
        include_file_remotes: bool,
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

        sync_snapshot_tree(&base)?;
        Ok(Self {
            base,
            project,
            prep,
            prep_existed,
            file_remotes,
        })
    }

    fn open(
        props: &ProjectProperties,
        base: PathBuf,
        prep_existed: bool,
        file_remotes: Vec<FileRemoteSnapshot>,
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
        Ok(())
    }

    fn restore_project_and_prep_durable(&self, props: &ProjectProperties) -> Result<()> {
        self.restore_project_and_prep(props)?;
        sync_restored_project_and_prep(props)
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

fn sync_restored_project_and_prep(props: &ProjectProperties) -> Result<()> {
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
    pub commit: TransactionCommit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRendererPayload {
    pub operation: String,
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
            },
        ));
        transaction.persist(props)?;
        let snapshot = match SyncSnapshot::capture_cancellable(
            props,
            snapshot_path,
            false,
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
        let manifest = capture_product_manifest(props)?;
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
        if self.0.status != TransactionStatus::SidecarCommitted {
            return Err(TeamError::Command(format!(
                "transaction {} is not awaiting renderer acknowledgement",
                self.0.batch_id
            )));
        }
        let commit = self.0.commit.clone().ok_or_else(|| {
            TeamError::Command(format!(
                "transaction {} has no product receipt",
                self.0.batch_id
            ))
        })?;
        Ok(TransactionRendererReceipt {
            version: self.0.version,
            project_root: self.0.project_root.clone(),
            generation: self.0.generation,
            batch_id: self.0.batch_id.clone(),
            status: self.0.status,
            error_code: self.0.error_code,
            updated_unix_ms: self.0.updated_unix_ms,
            payload: TransactionRendererPayload {
                operation: self.operation.clone(),
            },
            commit,
        })
    }

    fn validate_repository_shape(&self, props: &ProjectProperties) -> Result<()> {
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
            matches!(
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

    fn load_receipt_head(props: &ProjectProperties) -> Result<Option<Self>> {
        Ok(load_product_journal(props)?
            .batches
            .into_iter()
            .find(|transaction| transaction.0.status == TransactionStatus::SidecarCommitted))
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
        remove_path(&self.snapshot)?;
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
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn resolve_cancellation_checkpoint(point: &str) -> Result<()> {
    if std::env::var("OMEGAT_TEST_RESOLVE_CANCELLATION_POINT").as_deref() != Ok(point) {
        return Ok(());
    }
    if let Some(trigger) = std::env::var_os("OMEGAT_TEST_RESOLVE_CANCELLATION_TRIGGER") {
        if !PathBuf::from(trigger).is_file() {
            return Ok(());
        }
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_RESOLVE_CANCELLATION_MARKER") else {
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
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
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
        remove_path(&transaction.snapshot)?;
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

fn capture_product_manifest(props: &ProjectProperties) -> Result<TeamProductManifest> {
    let mut files = Vec::new();
    collect_product_tree(&props.root, "project", true, &mut files)?;
    for (index, repo) in props.repositories.iter().enumerate() {
        if repo.repo_type != "file" || is_inplace(props, repo) {
            continue;
        }
        let remote = PathBuf::from(&repo.url);
        let prefix = format!("file-remote/{index}");
        if remote.exists() || remote.is_symlink() {
            collect_product_tree(&remote, &prefix, false, &mut files)?;
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
    if transaction.operation != "resolve-conflict"
        || transaction.0.error_code != Some(REQUEST_CANCELLED_CODE)
    {
        return Err(TeamError::Command(format!(
            "invalid pending cancellation {}",
            transaction.0.batch_id
        )));
    }
    transaction.validate_repository_shape(props)?;
    let snapshot = SyncSnapshot::open(
        props,
        transaction.snapshot.clone(),
        transaction.prep_existed,
        transaction.file_remotes.clone(),
    )?;
    snapshot.restore_project_and_prep_durable(props)?;
    transaction.finish(
        props,
        "renderer-cancelled-recovered",
        TransactionStatus::RequestCancelled,
        Some(REQUEST_CANCELLED_CODE),
    )?;
    Ok(true)
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
    if let Err(error) = snapshot.restore_project_and_prep(props) {
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
    )?;
    journal.phase = "mutating".into();
    journal.persist(props)?;
    let result = mutation(cancellation);
    match result {
        Ok(value) => {
            if let Err(error) = check_cancelled(cancellation) {
                snapshot.restore_project_and_prep(props)?;
                journal.finish_for_error(props, "rolled-back", &error)?;
                return Err(error);
            }
            product_transaction_checkpoint(operation, "before_atomic_publish")?;
            let await_renderer_ack = generation != 0 && batch_id.is_some();
            if let Err(error) =
                journal.publish_product_commit(props, "committed", await_renderer_ack)
            {
                snapshot.restore_project_and_prep(props)?;
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
            if let Err(rollback_error) = snapshot.restore_project_and_prep(props) {
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
        if let Err(rollback_error) = snapshot.restore_project_and_prep(props) {
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
        if let Err(rollback_error) = snapshot.restore_project_and_prep(props) {
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
            if let Err(rollback_error) = snapshot.restore_project_and_prep(props) {
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
            if let Err(rollback_error) = snapshot.restore_project_and_prep(props) {
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
                && transaction.0.status == TransactionStatus::Completed
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
    let Some(mut transaction) = SyncTransaction::load_receipt_head(props)? else {
        return Ok(None);
    };
    transaction.validate_repository_shape(props)?;
    if transaction.0.generation != generation {
        // Renderer adoption must not move this receipt behind or ahead of the
        // refresh backend. updated_unix_ms is the durable dispatcher key.
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
    let Some(transaction) = SyncTransaction::load_receipt_head(props)? else {
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
    if generation == 0 || batch_id.is_empty() || operation.is_empty() {
        return Err(TeamError::Command(
            "renderer acknowledgement requires generation, batch id, and operation".into(),
        ));
    }
    let _lock = acquire_project_transaction_lock(props)?;
    if let Some(mut transaction) = SyncTransaction::load_receipt_head(props)? {
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
        if transaction.0.status != TransactionStatus::SidecarCommitted {
            return Err(TeamError::Conflict(format!(
                "transaction {batch_id} is not awaiting renderer acknowledgement"
            )));
        }
        transaction.phase = "renderer-acknowledged".into();
        transaction.0.transition(TransactionStatus::Completed, None);
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
    let _lock = acquire_project_transaction_lock(props)?;
    let Some(mut transaction) = SyncTransaction::load_receipt(props, batch_id)? else {
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
    if &capture_product_manifest(props)? != committed_manifest {
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

    let snapshot = SyncSnapshot::open(
        props,
        transaction.snapshot.clone(),
        transaction.prep_existed,
        transaction.file_remotes.clone(),
    )?;
    snapshot.restore_project_and_prep_durable(props)?;
    resolve_cancellation_checkpoint("after_rollback_fsync")?;

    transaction.phase = "renderer-cancelled".into();
    transaction.0.transition(
        TransactionStatus::RequestCancelled,
        Some(REQUEST_CANCELLED_CODE),
    );
    transaction.persist(props)?;
    resolve_cancellation_checkpoint("after_terminal_queue_rename")?;
    transaction.cleanup(props)
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
