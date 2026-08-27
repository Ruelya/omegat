//! Java `HTTPRemoteRepository`.

use crate::error::{Result, TeamError};
use crate::i_remote_repository2::IRemoteRepository2;
use crate::mapping::{effective_mappings, file_name_from_url};
use crate::project_team_settings::repo_work_dir;
use crate::team_utils::{run_cmd, strip_slash, which};
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use std::path::{Path, PathBuf};

pub struct HTTPRemoteRepository;

impl IRemoteRepository2 for HTTPRemoteRepository {
    fn repo_type(&self) -> &'static str {
        "http"
    }
    fn prepare(&self, props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
        prepare(props, repo)
    }
    fn commit(&self, _props: &ProjectProperties, _repo: &RepositoryDef) -> Result<()> {
        Ok(())
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    std::fs::create_dir_all(&dir)?;
    let mappings = effective_mappings(repo);
    for m in &mappings {
        let name = {
            let repo_rel = strip_slash(&m.repository);
            if repo_rel.is_empty() || repo_rel == "/" {
                file_name_from_url(&repo.url)
            } else {
                repo_rel.to_string()
            }
        };
        let dest = dir.join(&name);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        download(&repo.url, &dest)?;
    }
    Ok(())
}

pub fn download(url: &str, dest: &Path) -> Result<()> {
    if let Some(path) = file_url_path(url) {
        std::fs::copy(path, dest)?;
        return Ok(());
    }
    let p = Path::new(url);
    if p.exists() {
        if p.is_file() {
            std::fs::copy(p, dest)?;
            return Ok(());
        }
        return Err(TeamError::Command(
            "http repository URL must point at a file".into(),
        ));
    }
    if !which("curl") {
        return Err(TeamError::Command(format!(
            "cannot download {url}: curl not installed and URL is not a local file"
        )));
    }
    run_cmd("curl", None, &["-fsSL", "-o", &dest.to_string_lossy(), url])?;
    Ok(())
}

fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    Some(PathBuf::from(rest))
}

/// Java `HTTPRemoteRepository.switchToVersion`: only `null` (latest) is supported.
pub fn switch_to_version(version: Option<&str>) -> Result<()> {
    if version.is_some() {
        return Err(TeamError::Command("Not supported".into()));
    }
    Ok(())
}

/// Java retrieve on HTTP 304 leaves the existing file bytes unchanged.
pub fn retrieve_skips_write(status: u16) -> bool {
    status == 304
}

/// Apply retrieve: 304 keeps `existing`; otherwise write `body`.
pub fn retrieve_with_status(status: u16, dest: &Path, existing: &str, body: &str) -> Result<()> {
    if retrieve_skips_write(status) {
        if !dest.exists() {
            std::fs::write(dest, existing)?;
        }
        return Ok(());
    }
    std::fs::write(dest, body)?;
    Ok(())
}
