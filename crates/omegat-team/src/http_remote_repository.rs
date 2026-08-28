//! Java `HTTPRemoteRepository`.

use crate::error::{Result, TeamError};
use crate::i_remote_repository2::IRemoteRepository2;
use crate::mapping::{effective_mappings, file_name_from_url};
use crate::project_team_settings::repo_work_dir;
use crate::team_utils::strip_slash;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    fn file_version(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        file: &str,
    ) -> Result<Option<String>> {
        let path = Path::new(file);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            repo_work_dir(props, repo).join(path)
        };
        path.exists().then(|| sha1_file(&path)).transpose()
    }
    fn switch_to_version(
        &self,
        props: &ProjectProperties,
        repo: &RepositoryDef,
        version: Option<&str>,
    ) -> Result<()> {
        switch_to_version(version)?;
        prepare(props, repo)
    }
}

pub fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    std::fs::create_dir_all(&dir)?;
    let etags_path = dir.join(".etags");
    let mut etags = load_etags(&etags_path)?;
    let mappings = effective_mappings(repo);
    for m in &mappings {
        let repo_rel = strip_slash(&m.repository);
        let name = {
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
        let url = mapped_url(&repo.url, repo_rel);
        let etag = etags.get(&name).map(String::as_str);
        if let Some(next) = retrieve(&url, &dest, etag)? {
            etags.insert(name, next);
        }
    }
    save_etags(&etags_path, &etags)?;
    Ok(())
}

pub fn download(url: &str, dest: &Path) -> Result<()> {
    retrieve(url, dest, None).map(|_| ())
}

/// Retrieve one HTTP mapping, preserving an existing product on `304`.
///
/// `Some(etag)` is returned when the server supplied a replacement validator;
/// `None` means either an unchanged response or no validator.
pub fn retrieve(url: &str, dest: &Path, current_etag: Option<&str>) -> Result<Option<String>> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(path) = file_url_path(url) {
        publish_copy(&path, dest)?;
        return Ok(None);
    }
    let p = Path::new(url);
    if p.exists() {
        if p.is_file() {
            publish_copy(p, dest)?;
            return Ok(None);
        }
        return Err(TeamError::Command(
            "http repository URL must point at a file".into(),
        ));
    }
    retrieve_http(url, dest, current_etag)
}

fn retrieve_http(url: &str, dest: &Path, current_etag: Option<&str>) -> Result<Option<String>> {
    let temporary = temporary_path(dest, "download");
    let headers = temporary_path(dest, "headers");
    let _ = std::fs::remove_file(&temporary);
    let _ = std::fs::remove_file(&headers);
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--location",
        "--output",
        temporary.to_string_lossy().as_ref(),
        "--dump-header",
        headers.to_string_lossy().as_ref(),
        "--write-out",
        "%{http_code}",
    ]);
    if let Some(etag) = current_etag {
        command.args(["--header", &format!("If-None-Match: {etag}")]);
    }
    let output = command.arg(url).output().map_err(|error| {
        TeamError::Command(format!("cannot execute curl for {url}: {error}"))
    })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temporary);
        let _ = std::fs::remove_file(&headers);
        return Err(TeamError::Command(format!(
            "curl failed for {url}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .map_err(|error| TeamError::Command(format!("invalid HTTP status for {url}: {error}")))?;
    let raw_headers = std::fs::read_to_string(&headers).unwrap_or_default();
    let _ = std::fs::remove_file(&headers);
    match status {
        200..=299 => {
            publish_temporary(&temporary, dest)?;
            Ok(header_value(&raw_headers, "etag"))
        }
        304 => {
            let _ = std::fs::remove_file(&temporary);
            Ok(None)
        }
        _ => {
            let _ = std::fs::remove_file(&temporary);
            Err(TeamError::Command(format!(
                "HTTP repository {url} returned status {status}"
            )))
        }
    }
}

fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    #[cfg(windows)]
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    Some(PathBuf::from(rest))
}

fn mapped_url(base: &str, repository: &str) -> String {
    if repository.is_empty()
        || file_url_path(base).is_some_and(|path| path.is_file())
        || Path::new(base).is_file()
    {
        return base.to_string();
    }
    if base.ends_with('/') {
        format!("{base}{repository}")
    } else {
        format!("{base}/{repository}")
    }
}

fn temporary_path(path: &Path, label: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    path.with_file_name(format!(".{file_name}.{}.{label}.tmp", std::process::id()))
}

fn publish_copy(source: &Path, dest: &Path) -> Result<()> {
    let temporary = temporary_path(dest, "copy");
    let _ = std::fs::remove_file(&temporary);
    std::fs::copy(source, &temporary)?;
    publish_temporary(&temporary, dest)
}

fn publish_temporary(temporary: &Path, dest: &Path) -> Result<()> {
    File::open(temporary)?.sync_all()?;
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(temporary, dest)?;
    sync_parent(dest)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<()> {
    Ok(())
}

fn header_value(headers: &str, wanted: &str) -> Option<String> {
    headers.lines().rev().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case(wanted)
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn load_etags(path: &Path) -> Result<BTreeMap<String, String>> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Ok(BTreeMap::new());
    };
    Ok(raw
        .lines()
        .filter(|line| !line.starts_with(['#', '!']))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect())
}

fn save_etags(path: &Path, etags: &BTreeMap<String, String>) -> Result<()> {
    let temporary = temporary_path(path, "etags");
    let _ = std::fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    for (key, value) in etags {
        writeln!(file, "{key}={value}")?;
    }
    file.sync_all()?;
    drop(file);
    publish_temporary(&temporary, path)
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = digest.finalize();
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(value)
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
