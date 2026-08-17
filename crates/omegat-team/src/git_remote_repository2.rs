//! Java `GITRemoteRepository2`.

use crate::error::Result;
use crate::git_credentials_provider;
use crate::i_remote_repository2::IRemoteRepository2;
use crate::project_team_settings::{is_inplace, repo_work_dir};
use crate::team_utils::run_git;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use std::path::Path;

pub struct GITRemoteRepository2;

impl IRemoteRepository2 for GITRemoteRepository2 {
    fn repo_type(&self) -> &'static str {
        "git"
    }
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        prepare(props, repo)
    }
    fn commit(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        commit_and_push(props, repo)
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if is_inplace(props, repo) {
        if props.root.join(".git").exists() {
            let _ = run_git(Some(&props.root), &["pull", "--ff-only"]);
        }
        return Ok(());
    }
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    if dir.join(".git").exists() {
        let _ = run_git(Some(&dir), &["fetch", "origin"]);
        let branch = repo
            .branch
            .clone()
            .unwrap_or_else(|| current_branch(&dir).unwrap_or_else(|_| "main".into()));
        let remote_ref = format!("origin/{branch}");
        if run_git(Some(&dir), &["rev-parse", "--verify", &remote_ref]).is_ok() {
            run_git(Some(&dir), &["reset", "--hard", &remote_ref])?;
        } else {
            let _ = run_git(Some(&dir), &["pull", "--ff-only"]);
        }
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut args = vec!["clone".to_string()];
    if let Some(b) = &repo.branch {
        args.push("--branch".into());
        args.push(b.clone());
    }
    args.push(repo.url.clone());
    args.push(dir.to_string_lossy().into_owned());
    let user = git_credentials_provider::for_repo(props, repo);
    let mut with_cred = git_credentials_provider::git_config_args(&user);
    with_cred.extend(args);
    let args_ref: Vec<&str> = with_cred.iter().map(|s| s.as_str()).collect();
    run_git(None, &args_ref)?;
    Ok(())
}

pub fn commit_and_push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let dir = repo_work_dir(props, repo);
    if !dir.join(".git").exists() {
        return Ok(());
    }
    let _ = run_git(Some(&dir), &["add", "-A"]);
    let _ = commit(&dir, "OmegaT team sync");
    if is_inplace(props, repo) {
        let _ = run_git(Some(&dir), &["push"]);
        return Ok(());
    }
    let branch = repo
        .branch
        .clone()
        .unwrap_or_else(|| current_branch(&dir).unwrap_or_else(|_| "main".into()));
    let dest = format!("HEAD:refs/heads/{branch}");
    let _ = run_git(Some(&dir), &["push", "origin", &dest]);
    Ok(())
}

pub fn commit(dir: &Path, message: &str) -> Result<String> {
    run_git(
        Some(dir),
        &[
            "-c",
            "user.email=omegat@example.com",
            "-c",
            "user.name=OmegaT",
            "commit",
            "-m",
            message,
        ],
    )
}

pub fn current_branch(dir: &Path) -> Result<String> {
    Ok(run_git(Some(dir), &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string())
}
