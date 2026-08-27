//! Java `SVNRemoteRepository2`.

use crate::error::{Result, TeamError};
use crate::i_remote_repository2::IRemoteRepository2;
use crate::project_team_settings::repo_work_dir;
use crate::svn_authentication_manager;
use crate::team_utils::{run_cmd, which};
use omegat_core::properties::{ProjectProperties, RepositoryDef};

pub struct SVNRemoteRepository2;

impl IRemoteRepository2 for SVNRemoteRepository2 {
    fn repo_type(&self) -> &'static str {
        "svn"
    }
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        prepare(props, repo)
    }
    fn commit(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        commit(props, repo)
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if !which("svn") {
        return Err(TeamError::Command(
            "svn client not installed (STATUS: SVN tests require svn + svnadmin)".into(),
        ));
    }
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    let user = svn_authentication_manager::for_repo(props, repo);
    let auth = svn_authentication_manager::svn_auth_args(&user);
    if dir.join(".svn").exists() {
        let mut args = vec!["update".to_string()];
        args.extend(auth);
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_cmd("svn", Some(&dir), &refs)?;
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut args = vec![
        "checkout".to_string(),
        repo.url.clone(),
        dir.to_string_lossy().into_owned(),
    ];
    args.extend(auth);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_cmd("svn", None, &refs)?;
    Ok(())
}

pub fn commit(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if !which("svn") {
        return Err(TeamError::Command("svn client not installed".into()));
    }
    let dir = repo_work_dir(props, repo);
    if !dir.join(".svn").exists() {
        return Ok(());
    }
    let _ = run_cmd("svn", Some(&dir), &["add", "--force", "."]);
    let user = svn_authentication_manager::for_repo(props, repo);
    let mut args = vec!["commit".to_string(), "-m".into(), "OmegaT team sync".into()];
    args.extend(svn_authentication_manager::svn_auth_args(&user));
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let _ = run_cmd("svn", Some(&dir), &refs);
    Ok(())
}
