//! Java `RemoteRepositoryFactory`.

use crate::error::{Result, TeamError};
use crate::file_repository::FileRepository;
use crate::git_remote_repository2::GITRemoteRepository2;
use crate::http_remote_repository::HTTPRemoteRepository;
use crate::i_remote_repository2::IRemoteRepository2;
use crate::svn_remote_repository2::SVNRemoteRepository2;
use omegat_core::properties::{ProjectProperties, RepositoryDef};

/// Java `RemoteRepositoryFactory.detectRepositoryType`.
pub fn detect_repository_type(url: &str) -> Option<&'static str> {
    if url.starts_with("svn") {
        Some("svn")
    } else if url.starts_with("git") {
        Some("git")
    } else if url.starts_with("https://git") {
        Some("git")
    } else if url.ends_with(".git") {
        Some("git")
    } else {
        None
    }
}

pub fn create(repo_type: &str) -> Result<Box<dyn IRemoteRepository2>> {
    let repo: Box<dyn IRemoteRepository2> = match repo_type {
        "git" => Box::new(GITRemoteRepository2),
        "svn" => Box::new(SVNRemoteRepository2),
        "http" => Box::new(HTTPRemoteRepository),
        "file" => Box::new(FileRepository),
        other => return Err(TeamError::Unsupported(other.into())),
    };
    if repo.repo_type() != repo_type {
        return Err(TeamError::Unsupported(format!(
            "factory type {} != {}",
            repo.repo_type(),
            repo_type
        )));
    }
    Ok(repo)
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    create(&repo.repo_type)?.prepare(props, repo)
}

pub fn commit(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    create(&repo.repo_type)?.commit(props, repo)
}

pub fn file_version(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    file: &str,
) -> Result<Option<String>> {
    create(&repo.repo_type)?.file_version(props, repo, file)
}

pub fn switch_to_version(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    version: Option<&str>,
) -> Result<()> {
    create(&repo.repo_type)?.switch_to_version(props, repo, version)
}

pub fn recently_deleted_files(
    props: &ProjectProperties,
    repo: &RepositoryDef,
) -> Result<Vec<String>> {
    create(&repo.repo_type)?.recently_deleted_files(props, repo)
}

pub fn commit_after_versions(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    on_versions: &[Option<String>],
    comment: &str,
) -> Result<Option<String>> {
    create(&repo.repo_type)?.commit_after_versions(props, repo, on_versions, comment)
}
