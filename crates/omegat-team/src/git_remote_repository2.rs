//! Java `GITRemoteRepository2`.

use crate::error::Result;
use crate::git2_ops;
use crate::git_credentials_provider;
use crate::i_remote_repository2::IRemoteRepository2;
use crate::project_team_settings::{
    is_inplace, last_delete_check, repo_work_dir, set_last_delete_check,
};
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
    fn file_version(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        file: &str,
    ) -> Result<Option<String>> {
        git2_ops::file_version(&repo_work_dir(props, repo), file)
    }
    fn switch_to_version(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        version: Option<&str>,
    ) -> Result<()> {
        switch_to_version(props, repo, version)
    }
    fn recently_deleted_files(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
    ) -> Result<Vec<String>> {
        recently_deleted_files(props, repo)
    }
    fn commit_after_versions(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        on_versions: &[Option<String>],
        comment: &str,
    ) -> Result<Option<String>> {
        commit_and_push_after(props, repo, on_versions, comment)
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let user = git_credentials_provider::for_repo(props, repo);
    if is_inplace(props, repo) {
        if props.root.join(".git").exists() {
            git2_ops::pull_ff(&props.root, &user)?;
            git2_ops::update_submodules(&props.root, &user)?;
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
        git2_ops::update_submodules(&dir, &user)?;
        return Ok(());
    }
    git2_ops::clone(&repo.url, &dir, repo.branch.as_deref(), &user)?;
    git2_ops::update_submodules(&dir, &user)?;
    Ok(())
}

pub fn commit_and_push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    commit_and_push_after(props, repo, &[], "OmegaT team sync").map(|_| ())
}

pub fn commit_and_push_after(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    on_versions: &[Option<String>],
    message: &str,
) -> Result<Option<String>> {
    let dir = repo_work_dir(props, repo);
    if !dir.join(".git").exists() {
        return Err(crate::error::TeamError::Command(format!(
            "git2: repository is not initialized at {}",
            dir.display()
        )));
    }
    let user = git_credentials_provider::for_repo(props, repo);
    let expected: Vec<String> = on_versions.iter().flatten().cloned().collect();
    let committed = git2_ops::commit_if_changed(
        &dir,
        (!expected.is_empty()).then_some(expected.as_slice()),
        message,
    )?;
    let Some(version) = committed else {
        return Ok(None);
    };
    if is_inplace(props, repo) {
        let branch = git2_ops::current_branch(&dir)?;
        let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
        git2_ops::push(&dir, "origin", &spec, &user)?;
        return Ok(Some(version));
    }
    let branch = match &repo.branch {
        Some(branch) => branch.clone(),
        None => git2_ops::current_branch(&dir)?,
    };
    let dest = format!("HEAD:refs/heads/{branch}");
    git2_ops::push(&dir, "origin", &dest, &user)?;
    Ok(Some(version))
}

pub fn switch_to_version(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    version: Option<&str>,
) -> Result<()> {
    let dir = repo_work_dir(props, repo);
    let user = git_credentials_provider::for_repo(props, repo);
    let branch = match &repo.branch {
        Some(branch) => branch.clone(),
        None => git2_ops::current_branch(&dir)?,
    };
    let spec = if let Some(version) = version {
        version.to_string()
    } else {
        git2_ops::fetch(&dir, "origin", &user)?;
        let remote_ref = format!("refs/remotes/origin/{branch}");
        if !git2_ops::has_ref(&dir, &remote_ref) {
            return Err(crate::error::TeamError::Command(format!(
                "git2: configured branch origin/{branch} does not exist"
            )));
        }
        remote_ref
    };
    git2_ops::checkout_version(&dir, &spec, &branch)?;
    git2_ops::update_submodules(&dir, &user)?;
    Ok(())
}

pub fn recently_deleted_files(
    props: &ProjectProperties,
    repo: &RepositoryDef,
) -> Result<Vec<String>> {
    let previous = last_delete_check(props, repo)?;
    let (head, deleted) =
        git2_ops::recently_deleted_since(&repo_work_dir(props, repo), previous.as_deref())?;
    set_last_delete_check(props, repo, &head)?;
    Ok(deleted)
}

pub fn commit(dir: &Path, message: &str) -> Result<String> {
    git2_ops::commit(dir, message)
}

pub fn current_branch(dir: &Path) -> Result<String> {
    git2_ops::current_branch(dir)
}
