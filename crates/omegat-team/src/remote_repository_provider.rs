//! Java `RemoteRepositoryProvider`.

use crate::error::{Result, TeamError};
use crate::mapping::{copy_mapped, effective_mappings, propagate_deleted, CopyDir};
use crate::project_team_settings::{is_inplace, prep_dir};
use crate::rebase_and_commit::rebase_all;
use crate::rebase_utils::save_bases;
use crate::remote_repository_factory;
use crate::team_settings::{clear_resolved, save_conflicts};
use crate::{team_enabled, SyncReport};
use omegat_core::properties::ProjectProperties;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FileRemoteSnapshot {
    repository_index: usize,
    source: PathBuf,
    backup: PathBuf,
    is_file: bool,
}

struct SyncSnapshot {
    base: PathBuf,
    project: PathBuf,
    prep: PathBuf,
    prep_existed: bool,
    file_remotes: Vec<FileRemoteSnapshot>,
}

impl SyncSnapshot {
    fn capture(props: &ProjectProperties) -> Result<Self> {
        let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "omegat-team-sync-{}-{sequence}",
            std::process::id()
        ));
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
            if !source.exists() {
                continue;
            }
            let backup = base.join("file-remotes").join(repository_index.to_string());
            let is_file = source.is_file();
            if is_file {
                if let Some(parent) = backup.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&source, &backup)?;
            } else {
                crate::team_utils::copy_tree(&source, &backup, false)?;
            }
            file_remotes.push(FileRemoteSnapshot {
                repository_index,
                source,
                backup,
                is_file,
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

impl Drop for SyncSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
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
    commit_started: &[usize],
) -> Vec<String> {
    let mut failures = Vec::new();
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
        }
    }
    for &index in commit_started
        .iter()
        .filter(|index| !published.contains(index))
    {
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

pub fn sync(props: &ProjectProperties) -> Result<SyncReport> {
    if !team_enabled() {
        return Ok(SyncReport {
            action: "skipped".into(),
            message: "--no-team".into(),
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

    let snapshot = SyncSnapshot::capture(props)?;
    let mut observed = vec![None; props.repositories.len()];
    let mut rollback_versions = vec![None; props.repositories.len()];
    let mut published = Vec::new();
    let mut commit_started = Vec::new();
    let mut pending_conflicts = None;
    let transaction = (|| -> Result<()> {
        std::fs::create_dir_all(prep_dir(props))?;
        for repo in &props.repositories {
            remote_repository_factory::prepare(props, repo)?;
        }
        let mut deleted = Vec::with_capacity(props.repositories.len());
        for (index, repo) in props.repositories.iter().enumerate() {
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
        }
        for (repo, deleted) in props.repositories.iter().zip(&deleted) {
            copy_mapped(props, repo, CopyDir::RepoToProject)?;
            propagate_deleted(props, repo, deleted)?;
        }
        let conflicts = rebase_all(props)?;
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
        for repo in &props.repositories {
            copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
        }
        for (index, repo) in props.repositories.iter().enumerate() {
            commit_started.push(index);
            remote_repository_factory::commit_after_versions(
                props,
                repo,
                &[observed[index].clone()],
                "OmegaT team sync",
            )?;
            published.push(index);
        }
        save_bases(props)?;
        save_conflicts(props, &[])?;
        clear_resolved(props);
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
            return Err(error);
        }
        return Err(TeamError::Command(format!(
            "{error}; rollback failed: {}",
            rollback_failures.join(" | ")
        )));
    }

    for repo in &props.repositories {
        report
            .message
            .push_str(&format!("synced {}; ", repo.repo_type));
    }
    Ok(report)
}

pub fn commit_project_files(props: &ProjectProperties, which: &str) -> Result<SyncReport> {
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
        let snapshot = SyncSnapshot::capture(props)?;
        let mut rollback_versions = vec![None; props.repositories.len()];
        let mut published = Vec::new();
        let mut commit_started = Vec::new();
        let transaction = (|| -> Result<()> {
            for (index, repo) in props.repositories.iter().enumerate() {
                if repo.repo_type == "git" {
                    rollback_versions[index] =
                        remote_repository_factory::file_version(props, repo, "")?;
                }
            }
            for repo in &props.repositories {
                copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
            }
            for (index, repo) in props.repositories.iter().enumerate() {
                commit_started.push(index);
                remote_repository_factory::commit_after_versions(
                    props,
                    repo,
                    &[rollback_versions[index].clone()],
                    &format!("OmegaT commit {label} files"),
                )?;
                published.push(index);
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
                return Err(error);
            }
            return Err(TeamError::Command(format!(
                "{error}; rollback failed: {}",
                rollback_failures.join(" | ")
            )));
        }
    } else if props.root.join(".git").exists() {
        crate::git2_ops::add_all(&props.root)?;
        crate::git_remote_repository2::commit(
            &props.root,
            &format!("OmegaT commit {label} files"),
        )?;
    }
    Ok(SyncReport {
        action: format!("commit-{label}"),
        message: format!("committed {label} under {}", dir.display()),
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
    let repo = props.repositories.get(repository_index).ok_or_else(|| {
        TeamError::Command(format!(
            "repository index {repository_index} is out of range"
        ))
    })?;
    remote_repository_factory::commit_after_versions(props, repo, versions, comment)
}
