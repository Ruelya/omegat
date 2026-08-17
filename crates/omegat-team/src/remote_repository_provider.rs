//! Java `RemoteRepositoryProvider`.

use crate::error::{Result, TeamError};
use crate::mapping::{copy_mapped, CopyDir};
use crate::project_team_settings::prep_dir;
use crate::rebase_and_commit::rebase_all;
use crate::rebase_utils::save_bases;
use crate::remote_repository_factory;
use crate::team_settings::{clear_resolved, save_conflicts};
use crate::{SyncReport, team_enabled};
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
        copy_mapped(props, repo, CopyDir::RepoToProject)?;
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
        remote_repository_factory::commit(props, repo)?;
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
        let _ = crate::git2_ops::add_all(&props.root);
        let _ = crate::git_remote_repository2::commit(&props.root, &format!("OmegaT commit {label} files"));
    }
    Ok(SyncReport {
        action: format!("commit-{label}"),
        message: format!("committed {label} under {}", dir.display()),
        conflicts: vec![],
    })
}
