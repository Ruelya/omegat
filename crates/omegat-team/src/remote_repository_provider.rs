//! Java `RemoteRepositoryProvider`.

use crate::error::{Result, TeamError};
use crate::mapping::{copy_mapped, effective_mappings, propagate_deleted, CopyDir};
use crate::project_team_settings::prep_dir;
use crate::rebase_and_commit::rebase_all;
use crate::rebase_utils::save_bases;
use crate::remote_repository_factory;
use crate::team_settings::{clear_resolved, save_conflicts};
use crate::{team_enabled, SyncReport};
use omegat_core::properties::ProjectProperties;

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
    std::fs::create_dir_all(prep_dir(props))?;
    for repo in &props.repositories {
        remote_repository_factory::prepare(props, repo)?;
        let mappings = effective_mappings(repo);
        let version_path = mappings
            .first()
            .map(|mapping| mapping.repository.trim_matches(['/', '\\']))
            .unwrap_or("");
        let observed = remote_repository_factory::file_version(props, repo, version_path)?;
        copy_mapped(props, repo, CopyDir::RepoToProject)?;
        let deleted = remote_repository_factory::recently_deleted_files(props, repo)?;
        propagate_deleted(props, repo, &deleted)?;
        let conflicts = rebase_all(props)?;
        if !conflicts.is_empty() {
            save_conflicts(props, &conflicts)?;
            return Err(TeamError::Conflict(
                conflicts
                    .iter()
                    .map(|c| format!("{}:{}", c.kind, c.source))
                    .collect::<Vec<_>>()
                    .join(" | "),
            ));
        }
        copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
        remote_repository_factory::commit_after_versions(
            props,
            repo,
            &[observed],
            "OmegaT team sync",
        )?;
        save_bases(props)?;
        clear_resolved(props);
        report
            .message
            .push_str(&format!("synced {}; ", repo.repo_type));
    }
    save_conflicts(props, &[])?;
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
    for repo in &props.repositories {
        copy_mapped(props, repo, CopyDir::ProjectToRepo)?;
        remote_repository_factory::commit(props, repo)?;
    }
    if props.repositories.is_empty() && props.root.join(".git").exists() {
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
