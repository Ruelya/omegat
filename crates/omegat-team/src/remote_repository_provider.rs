//! Java `RemoteRepositoryProvider`.

use crate::error::{Result, TeamError};
use crate::mapping::{copy_mapped, effective_mappings, propagate_deleted, CopyDir};
use crate::project_team_settings::{is_inplace, prep_dir};
use crate::rebase_and_commit::rebase_all;
use crate::rebase_utils::save_bases;
use crate::remote_repository_factory;
use crate::team_settings::{clear_resolved, save_conflicts};
use crate::{team_enabled, SyncReport};
use fs2::FileExt;
use omegat_core::cancellation::CancellationToken;
use omegat_core::properties::ProjectProperties;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static FAIL_COMMIT_REPOSITORY: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(test)]
static CRASH_AFTER_PUBLISH_REPOSITORY: AtomicUsize = AtomicUsize::new(usize::MAX);

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
pub(crate) fn crash_after_publish_for(repository_index: usize) {
    CRASH_AFTER_PUBLISH_REPOSITORY.store(repository_index, Ordering::SeqCst);
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
        if base.exists() {
            std::fs::remove_dir_all(&base)?;
        }
        let project = base.join("project");
        crate::team_utils::copy_tree(&props.root, &project, true)?;

        let prep_source = prep_dir(props);
        let prep = base.join("prep");
        let prep_existed = prep_source.exists();
        if prep_existed {
            crate::team_utils::copy_tree(&prep_source, &prep, false)?;
        }

        let mut file_remotes = Vec::new();
        for (repository_index, repo) in props.repositories.iter().enumerate() {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SyncTransaction {
    format: u8,
    id: String,
    operation: String,
    phase: String,
    snapshot: PathBuf,
    prep_existed: bool,
    file_remotes: Vec<FileRemoteSnapshot>,
    repository_count: usize,
    rollback_versions: Vec<Option<String>>,
    commit_started: Vec<usize>,
    published: Vec<usize>,
    updated_unix_ms: u128,
}

impl SyncTransaction {
    fn begin(props: &ProjectProperties, operation: &str) -> Result<(Self, SyncSnapshot)> {
        let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{}-{sequence}", unix_ms(), std::process::id());
        let snapshot_path = transaction_dir(props).join(format!("{id}.snapshot"));
        let snapshot = SyncSnapshot::capture(props, snapshot_path.clone())?;
        let mut transaction = Self {
            format: 1,
            id,
            operation: operation.into(),
            phase: "captured".into(),
            snapshot: snapshot_path,
            prep_existed: snapshot.prep_existed,
            file_remotes: snapshot.file_remotes.clone(),
            repository_count: props.repositories.len(),
            rollback_versions: vec![None; props.repositories.len()],
            commit_started: Vec::new(),
            published: Vec::new(),
            updated_unix_ms: unix_ms(),
        };
        transaction.persist(props)?;
        Ok((transaction, snapshot))
    }

    fn persist(&mut self, props: &ProjectProperties) -> Result<()> {
        self.updated_unix_ms = unix_ms();
        let dir = transaction_dir(props);
        std::fs::create_dir_all(&dir)?;
        let active = dir.join("active.json");
        let temporary = dir.join(format!(".active-{}.tmp", self.id));
        let json = serde_json::to_vec_pretty(self)
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&json)?;
            file.sync_all()?;
        }
        let previous = dir.join(".active.previous.json");
        remove_path(&previous)?;
        if active.exists() {
            std::fs::rename(&active, &previous)?;
        }
        if let Err(error) = std::fs::rename(&temporary, &active) {
            if previous.exists() {
                let _ = std::fs::rename(&previous, &active);
            }
            return Err(error.into());
        }
        remove_path(&previous)?;
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

    fn finish(mut self, props: &ProjectProperties, phase: &str) -> Result<()> {
        self.phase = phase.into();
        self.persist(props)?;
        remove_path(&transaction_dir(props).join("active.json"))?;
        remove_path(&transaction_dir(props).join(".active.previous.json"))?;
        remove_path(&self.snapshot)?;
        Ok(())
    }

    fn load(props: &ProjectProperties) -> Result<Option<Self>> {
        let dir = transaction_dir(props);
        let active = dir.join("active.json");
        let previous = dir.join(".active.previous.json");
        let path = if active.is_file() {
            active
        } else if previous.is_file() {
            previous
        } else {
            return Ok(None);
        };
        let transaction: Self = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .map_err(|error| TeamError::Command(format!("team transaction: {error}")))?;
        if transaction.format != 1 {
            return Err(TeamError::Command(format!(
                "unsupported team transaction format {}",
                transaction.format
            )));
        }
        Ok(Some(transaction))
    }
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

fn recover_interrupted_sync_locked(props: &ProjectProperties) -> Result<bool> {
    let Some(mut transaction) = SyncTransaction::load(props)? else {
        return Ok(false);
    };
    if transaction.repository_count != props.repositories.len()
        || transaction.rollback_versions.len() != props.repositories.len()
    {
        return Err(TeamError::Command(format!(
            "team transaction {} expected {} repositories, found {}",
            transaction.id,
            transaction.repository_count,
            props.repositories.len()
        )));
    }
    let snapshot = SyncSnapshot::open(
        props,
        transaction.snapshot.clone(),
        transaction.prep_existed,
        transaction.file_remotes.clone(),
    )?;
    transaction.phase = "recovering".into();
    for &index in &transaction.commit_started {
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
    if let Err(error) = snapshot.restore_project_and_prep(props) {
        failures.push(format!("project: {error}"));
    }
    if failures.is_empty() {
        transaction.finish(props, "recovered")?;
        return Ok(true);
    }
    transaction.phase = "recovery-failed".into();
    let _ = transaction.persist(props);
    Err(TeamError::Command(format!(
        "team transaction recovery failed: {}",
        failures.join(" | ")
    )))
}

pub fn sync(props: &ProjectProperties) -> Result<SyncReport> {
    sync_cancellable(props, &CancellationToken::default())
}

pub fn sync_cancellable(
    props: &ProjectProperties,
    cancellation: &CancellationToken,
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

    let (mut journal, snapshot) = SyncTransaction::begin(props, "sync")?;
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
            copy_mapped(props, repo, CopyDir::RepoToProject)?;
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
            copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
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
            journal.finish(props, "rolled-back")?;
            return Err(error);
        }
        journal.phase = "rollback-failed".into();
        let _ = journal.persist(props);
        return Err(TeamError::Command(format!(
            "{error}; rollback failed: {}",
            rollback_failures.join(" | ")
        )));
    }

    journal.finish(props, "committed")?;
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
    if !props.repositories.is_empty() {
        let (mut journal, snapshot) = SyncTransaction::begin(props, &format!("commit-{label}"))?;
        let mut rollback_versions = vec![None; props.repositories.len()];
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
            journal.rollback_versions.clone_from(&rollback_versions);
            journal.phase = "staging".into();
            journal.persist(props)?;
            for repo in &props.repositories {
                check_cancelled(cancellation)?;
                copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
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
            if rollback_failures.is_empty() {
                journal.finish(props, "rolled-back")?;
                return Err(error);
            }
            journal.phase = "rollback-failed".into();
            let _ = journal.persist(props);
            return Err(TeamError::Command(format!(
                "{error}; rollback failed: {}",
                rollback_failures.join(" | ")
            )));
        }
        journal.finish(props, "committed")?;
    } else if props.root.join(".git").exists() {
        check_cancelled(cancellation)?;
        crate::git2_ops::add_all(&props.root)?;
        check_cancelled(cancellation)?;
        crate::git_remote_repository2::commit(
            &props.root,
            &format!("OmegaT commit {label} files"),
        )?;
        check_cancelled(cancellation)?;
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
