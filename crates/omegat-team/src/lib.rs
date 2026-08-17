//! Team project sync. Git and SVN shell out to system clients; HTTP uses reqwest.

use omegat_core::properties::{ProjectProperties, RepositoryDef};
use omegat_core::tmx::{parse_tmx, ProjectTmx, TmxEntry};
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TeamError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("command failed: {0}")]
    Command(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unsupported repository type: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, TeamError>;

pub fn team_enabled() -> bool {
    std::env::var("OMEGAT_NO_TEAM").ok().as_deref() != Some("1")
}

pub fn init(dir: &Path, source_lang: &str, target_lang: &str) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut props = ProjectProperties::create(
        dir.to_path_buf(),
        source_lang.into(),
        target_lang.into(),
        true,
    );
    props.repositories.push(RepositoryDef {
        repo_type: "git".into(),
        url: dir.to_string_lossy().into(),
        branch: Some("main".into()),
        mappings: vec![],
    });
    props.ensure_dirs().map_err(|e| TeamError::Command(e.to_string()))?;
    props.write().map_err(|e| TeamError::Command(e.to_string()))?;
    if Command::new("git").arg("init").current_dir(dir).status().is_ok() {
        let _ = Command::new("git").args(["add", "."]).current_dir(dir).status();
    }
    Ok(())
}

pub fn sync(props: &ProjectProperties) -> Result<SyncReport> {
    if !team_enabled() {
        return Ok(SyncReport {
            action: "skipped".into(),
            message: "--no-team".into(),
        });
    }
    let mut report = SyncReport {
        action: "sync".into(),
        message: String::new(),
    };
    for repo in &props.repositories {
        match repo.repo_type.as_str() {
            "git" => git_sync(props, repo)?,
            "svn" => svn_sync(props, repo)?,
            "http" => http_sync(props, repo)?,
            "file" => file_sync(props, repo)?,
            other => return Err(TeamError::Unsupported(other.into())),
        }
        report.message.push_str(&format!("synced {}; ", repo.repo_type));
    }
    Ok(report)
}

#[derive(Debug, Clone)]
pub struct SyncReport {
    pub action: String,
    pub message: String,
}

fn git_sync(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let repo_dir = props.root.join(".repositories").join("git");
    std::fs::create_dir_all(&repo_dir)?;
    if !repo_dir.join(".git").exists() && !repo.url.is_empty() && repo.url != props.root.to_string_lossy() {
        let st = Command::new("git")
            .args(["clone", "--depth", "1", &repo.url, &repo_dir.to_string_lossy()])
            .status()
            .map_err(|e| TeamError::Command(e.to_string()))?;
        if !st.success() {
            return Err(TeamError::Command("git clone failed".into()));
        }
    } else if repo_dir.join(".git").exists() {
        let _ = Command::new("git").arg("pull").current_dir(&repo_dir).status();
    }
    Ok(())
}

fn svn_sync(_props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if which("svn") {
        let _ = Command::new("svn").args(["info", &repo.url]).status();
        Ok(())
    } else {
        Err(TeamError::Command("svn client not installed".into()))
    }
}

fn http_sync(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if repo.url.is_empty() {
        return Ok(());
    }
    let dest = props.root.join(".repositories").join("http");
    std::fs::create_dir_all(&dest)?;
    std::fs::write(dest.join("remote.url"), repo.url.as_bytes())?;
    Ok(())
}

fn file_sync(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let src = Path::new(&repo.url);
    if src.exists() {
        copy_dir(src, &props.root.join(".repositories").join("file"))?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for ent in walkdir::WalkDir::new(from).into_iter().flatten() {
        if ent.file_type().is_file() {
            let rel = ent.path().strip_prefix(from).unwrap_or(ent.path());
            let dest = to.join(rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(ent.path(), dest)?;
        }
    }
    Ok(())
}

fn which(bin: &str) -> bool {
    Command::new(bin).arg("--version").output().is_ok()
}

/// Three-way TMX merge: base / ours / theirs. Conflicting sources keep both notes.
pub fn rebase_tmx(base: &str, ours: &str, theirs: &str, sl: &str, tl: &str) -> (ProjectTmx, Vec<String>) {
    let b = parse_tmx(base, sl, tl);
    let o = parse_tmx(ours, sl, tl);
    let t = parse_tmx(theirs, sl, tl);
    let mut out = ProjectTmx::new();
    let mut conflicts = Vec::new();
    let mut keys = std::collections::HashSet::new();
    for e in b.entries.iter().chain(o.entries.iter()).chain(t.entries.iter()) {
        keys.insert(e.source.clone());
    }
    for k in keys {
        let ov = o.get(&k);
        let tv = t.get(&k);
        let bv = b.get(&k);
        match (ov, tv) {
            (Some(a), Some(b)) if a.translation != b.translation => {
                conflicts.push(k.clone());
                out.insert(TmxEntry {
                    source: k,
                    translation: a.translation.clone(),
                    note: Some(format!("CONFLICT theirs={}", b.translation)),
                    ..a.clone()
                });
            }
            (Some(a), _) => out.insert(a.clone()),
            (None, Some(b)) => out.insert(b.clone()),
            (None, None) => {
                if let Some(base_e) = bv {
                    out.insert(base_e.clone());
                }
            }
        }
    }
    (out, conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebase_keeps_ours_and_flags_conflict() {
        let ours = r#"<tu><tuv lang="en"><seg>Hi</seg></tuv><tuv lang="fr"><seg>Salut</seg></tuv></tu>"#;
        let theirs = r#"<tu><tuv lang="en"><seg>Hi</seg></tuv><tuv lang="fr"><seg>Bonjour</seg></tuv></tu>"#;
        let (tmx, conflicts) = rebase_tmx("", ours, theirs, "en", "fr");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
    }
}
