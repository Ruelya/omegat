//! Java `FileRepository`.

use crate::error::{Result, TeamError};
use crate::i_remote_repository2::IRemoteRepository2;
use crate::project_team_settings::repo_work_dir;
use crate::team_utils::copy_tree;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use std::path::Path;

pub struct FileRepository;

impl IRemoteRepository2 for FileRepository {
    fn repo_type(&self) -> &'static str {
        "file"
    }
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        prepare(props, repo)
    }
    fn commit(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        push(props, repo)
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let src = Path::new(&repo.url);
    if !src.exists() {
        return Err(TeamError::Command(format!(
            "file repository missing: {}",
            src.display()
        )));
    }
    let dir = repo_work_dir(props, repo);
    if src.is_file() {
        std::fs::create_dir_all(&dir)?;
        let name = src.file_name().unwrap_or_default();
        std::fs::copy(src, dir.join(name))?;
    } else {
        copy_tree(src, &dir, true)?;
    }
    Ok(())
}

pub fn push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let dest = Path::new(&repo.url);
    let dir = repo_work_dir(props, repo);
    if dest.is_file() || dest.extension().is_some() {
        if let Some(name) = dest.file_name() {
            let src = dir.join(name);
            if src.exists() {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(src, dest)?;
            }
        }
        return Ok(());
    }
    copy_tree(&dir, dest, true)?;
    Ok(())
}
