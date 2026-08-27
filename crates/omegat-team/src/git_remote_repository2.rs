//! Java `GITRemoteRepository2`.

use crate::error::Result;
use crate::git2_ops;
use crate::git_credentials_provider;
use crate::i_remote_repository2::IRemoteRepository2;
use crate::project_team_settings::{is_inplace, repo_work_dir};
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
    let user = git_credentials_provider::for_repo(props, repo);
    if is_inplace(props, repo) {
        if props.root.join(".git").exists() {
            git2_ops::pull_ff(&props.root, &user)?;
        }
        return Ok(());
    }
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    if dir.join(".git").exists() {
        git2_ops::fetch(&dir, "origin", &user)?;
        let branch = match &repo.branch {
            Some(branch) => branch.clone(),
            None => git2_ops::current_branch(&dir)?,
        };
        let remote_ref = format!("refs/remotes/origin/{branch}");
        if !git2_ops::has_ref(&dir, &remote_ref) {
            return Err(crate::error::TeamError::Command(format!(
                "git2: configured branch origin/{branch} does not exist"
            )));
        }
        git2_ops::reset_hard(&dir, &remote_ref)?;
        return Ok(());
    }
    git2_ops::clone(&repo.url, &dir, repo.branch.as_deref(), &user)?;
    Ok(())
}

pub fn commit_and_push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let dir = repo_work_dir(props, repo);
    if !dir.join(".git").exists() {
        return Err(crate::error::TeamError::Command(format!(
            "git2: repository is not initialized at {}",
            dir.display()
        )));
    }
    let user = git_credentials_provider::for_repo(props, repo);
    let committed = git2_ops::commit_if_changed(&dir, None, "OmegaT team sync")?;
    if committed.is_none() {
        return Ok(());
    }
    if is_inplace(props, repo) {
        let branch = git2_ops::current_branch(&dir)?;
        let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
        return git2_ops::push(&dir, "origin", &spec, &user);
    }
    let branch = match &repo.branch {
        Some(branch) => branch.clone(),
        None => git2_ops::current_branch(&dir)?,
    };
    let dest = format!("HEAD:refs/heads/{branch}");
    git2_ops::push(&dir, "origin", &dest, &user)
}

pub fn commit(dir: &Path, message: &str) -> Result<String> {
    git2_ops::commit(dir, message)
}

pub fn current_branch(dir: &Path) -> Result<String> {
    git2_ops::current_branch(dir)
}
