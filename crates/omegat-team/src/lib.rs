//! Team project sync aligned with Java `RemoteRepositoryProvider` + `RebaseAndCommit`.
//!
//! Layout: `.repositories/<sanitized-url>/` is the remote working copy;
//! `.repositories/prep/` holds the last-synced TMX/glossary base and conflict list.
//! `sync` is prepare → rebase (TMX **and** glossary) → commit/push.

use omegat_core::glossary::{parse_glossary, GlossaryEntry};
use omegat_core::properties::{ProjectProperties, RepositoryDef, RepositoryMapping};
use omegat_core::tmx::{parse_tmx, ProjectTmx, TmxEntry};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub const REPO_SUBDIR: &str = ".repositories";
pub const REPO_PREP: &str = "prep";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conflict {
    pub kind: String,
    pub source: String,
    pub ours: String,
    pub theirs: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub action: String,
    pub message: String,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
}

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
        mappings: vec![default_mapping()],
    });
    props
        .ensure_dirs()
        .map_err(|e| TeamError::Command(e.to_string()))?;
    props
        .write()
        .map_err(|e| TeamError::Command(e.to_string()))?;
    if run_git(Some(dir), &["init"]).is_ok() {
        let _ = run_git(Some(dir), &["add", "-A"]);
        let _ = git_commit(dir, "OmegaT team init");
    }
    Ok(())
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
        match repo.repo_type.as_str() {
            "git" => git_commit_and_push(props, repo)?,
            "svn" => svn_commit(props, repo)?,
            "file" => file_push(props, repo)?,
            "http" => {}
            other => return Err(TeamError::Unsupported(other.into())),
        }
    }
    if props.repositories.is_empty() && props.root.join(".git").exists() {
        let _ = run_git(
            Some(&props.root),
            &["add", "-A", &dir.to_string_lossy()],
        );
        let _ = git_commit(&props.root, &format!("OmegaT commit {label} files"));
    }
    Ok(SyncReport {
        action: format!("commit-{label}"),
        message: format!("committed {label} under {}", dir.display()),
        conflicts: vec![],
    })
}

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
        prepare(props, repo)?;
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
        match repo.repo_type.as_str() {
            "git" => git_commit_and_push(props, repo)?,
            "svn" => svn_commit(props, repo)?,
            "file" => file_push(props, repo)?,
            "http" => {}
            other => return Err(TeamError::Unsupported(other.into())),
        }
        save_bases(props)?;
        clear_resolved(props);
        report
            .message
            .push_str(&format!("synced {}; ", repo.repo_type));
    }
    save_conflicts(props, &[])?;
    Ok(report)
}

pub fn list_conflicts(props: &ProjectProperties) -> Vec<Conflict> {
    read_json(&conflicts_path(props)).unwrap_or_default()
}

pub fn rebase_project(props: &ProjectProperties) -> Result<Vec<String>> {
    let conflicts = rebase_all(props)?;
    save_conflicts(props, &conflicts)?;
    Ok(conflicts.into_iter().map(|c| c.source).collect())
}

pub fn rebase_all(props: &ProjectProperties) -> Result<Vec<Conflict>> {
    let resolved = read_resolved(props);
    let mut conflicts = rebase_tmx_files(props, &resolved)?;
    conflicts.extend(rebase_glossary_files(props, &resolved)?);
    Ok(conflicts)
}

pub fn resolve(
    props: &ProjectProperties,
    source: &str,
    side: &str,
    translation: Option<&str>,
) -> Result<Vec<Conflict>> {
    let mut conflicts = list_conflicts(props);
    let Some(idx) = conflicts.iter().position(|c| c.source == source) else {
        return Ok(conflicts);
    };
    let chosen = match side {
        "theirs" => conflicts[idx].theirs.clone(),
        "manual" => translation.unwrap_or(&conflicts[idx].ours).to_string(),
        _ => conflicts[idx].ours.clone(),
    };
    let kind = conflicts[idx].kind.clone();
    if kind == "glossary" {
        apply_glossary_resolution(props, source, &chosen)?;
    } else {
        apply_tmx_resolution(props, source, &chosen)?;
    }
    mark_resolved(props, source);
    conflicts.remove(idx);
    save_conflicts(props, &conflicts)?;
    Ok(conflicts)
}

/// Three-way TMX merge: base / ours / theirs. Conflicting sources keep ours and record theirs.
pub fn rebase_tmx(
    base: &str,
    ours: &str,
    theirs: &str,
    sl: &str,
    tl: &str,
) -> (ProjectTmx, Vec<String>) {
    let (tmx, conflicts) = rebase_tmx_detailed(base, ours, theirs, sl, tl, &HashSet::new());
    (tmx, conflicts.into_iter().map(|c| c.source).collect())
}

fn rebase_tmx_detailed(
    base: &str,
    ours: &str,
    theirs: &str,
    sl: &str,
    tl: &str,
    resolved: &HashSet<String>,
) -> (ProjectTmx, Vec<Conflict>) {
    let b = parse_tmx(base, sl, tl);
    let o = parse_tmx(ours, sl, tl);
    let t = parse_tmx(theirs, sl, tl);
    let mut out = ProjectTmx::new();
    let mut conflicts = Vec::new();
    let mut keys = HashSet::new();
    for e in b.entries.iter().chain(o.entries.iter()).chain(t.entries.iter()) {
        keys.insert(e.source.clone());
    }
    for k in keys {
        let ov = o.get(&k);
        let tv = t.get(&k);
        let bv = b.get(&k);
        match (ov, tv) {
            (Some(a), Some(tb)) if a.translation != tb.translation => {
                if resolved.contains(&k) {
                    out.insert(a.clone());
                    continue;
                }
                let base_t = bv.map(|e| e.translation.as_str()).unwrap_or("");
                if !base_t.is_empty() && a.translation == base_t {
                    out.insert(tb.clone());
                } else if !base_t.is_empty() && tb.translation == base_t {
                    out.insert(a.clone());
                } else {
                    conflicts.push(Conflict {
                        kind: "tmx".into(),
                        source: k.clone(),
                        ours: a.translation.clone(),
                        theirs: tb.translation.clone(),
                        message: format!("TMX conflict on {k}"),
                    });
                    out.insert(TmxEntry {
                        source: k,
                        translation: a.translation.clone(),
                        note: Some(format!("CONFLICT theirs={}", tb.translation)),
                        ..a.clone()
                    });
                }
            }
            (Some(a), _) => out.insert(a.clone()),
            (None, Some(tb)) => out.insert(tb.clone()),
            (None, None) => {
                if let Some(base_e) = bv {
                    out.insert(base_e.clone());
                }
            }
        }
    }
    (out, conflicts)
}

fn rebase_tmx_files(props: &ProjectProperties, resolved: &HashSet<String>) -> Result<Vec<Conflict>> {
    let ours_path = props.save_tmx_path();
    let Some(theirs_path) = find_remote_tmx(props) else {
        return Ok(vec![]);
    };
    if !ours_path.exists() {
        if let Some(parent) = ours_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&theirs_path, &ours_path)?;
        return Ok(vec![]);
    }
    let ours = std::fs::read_to_string(&ours_path)?;
    let theirs = std::fs::read_to_string(&theirs_path)?;
    let base_s = std::fs::read_to_string(base_tmx_path(props)).unwrap_or_default();
    let (merged, conflicts) = rebase_tmx_detailed(
        &base_s,
        &ours,
        &theirs,
        &props.source_lang,
        &props.target_lang,
        resolved,
    );
    merged
        .write(&ours_path, &props.source_lang, &props.target_lang)
        .map_err(|e| TeamError::Command(e.to_string()))?;
    Ok(conflicts)
}

fn rebase_glossary_files(
    props: &ProjectProperties,
    resolved: &HashSet<String>,
) -> Result<Vec<Conflict>> {
    let ours_path = &props.glossary_file;
    let Some(theirs_path) = find_remote_glossary(props) else {
        return Ok(vec![]);
    };
    if !ours_path.exists() {
        if let Some(parent) = ours_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&theirs_path, ours_path)?;
        return Ok(vec![]);
    }
    let ours = parse_glossary(&std::fs::read_to_string(ours_path)?);
    let theirs = parse_glossary(&std::fs::read_to_string(&theirs_path)?);
    let base = parse_glossary(&std::fs::read_to_string(base_glossary_path(props)).unwrap_or_default());
    let (merged, conflicts) = rebase_glossary(&base, &ours, &theirs, resolved);
    write_glossary(ours_path, &merged)?;
    Ok(conflicts)
}

fn rebase_glossary(
    base: &[GlossaryEntry],
    ours: &[GlossaryEntry],
    theirs: &[GlossaryEntry],
    resolved: &HashSet<String>,
) -> (Vec<GlossaryEntry>, Vec<Conflict>) {
    let b = glossary_map(base);
    let o = glossary_map(ours);
    let t = glossary_map(theirs);
    let mut keys = HashSet::new();
    keys.extend(b.keys().cloned());
    keys.extend(o.keys().cloned());
    keys.extend(t.keys().cloned());
    let mut out = Vec::new();
    let mut conflicts = Vec::new();
    for k in keys {
        match (o.get(&k), t.get(&k)) {
            (Some(a), Some(tb)) if a.target != tb.target => {
                if resolved.contains(&k) {
                    out.push(a.clone());
                    continue;
                }
                let base_t = b.get(&k).map(|e| e.target.as_str()).unwrap_or("");
                if !base_t.is_empty() && a.target == base_t {
                    out.push(tb.clone());
                } else if !base_t.is_empty() && tb.target == base_t {
                    out.push(a.clone());
                } else {
                    conflicts.push(Conflict {
                        kind: "glossary".into(),
                        source: k,
                        ours: a.target.clone(),
                        theirs: tb.target.clone(),
                        message: format!("glossary conflict on {}", a.source),
                    });
                    out.push(a.clone());
                }
            }
            (Some(a), _) => out.push(a.clone()),
            (None, Some(tb)) => out.push(tb.clone()),
            (None, None) => {
                if let Some(be) = b.get(&k) {
                    out.push(be.clone());
                }
            }
        }
    }
    out.sort_by(|a, b| a.source.cmp(&b.source));
    (out, conflicts)
}

fn glossary_map(entries: &[GlossaryEntry]) -> HashMap<String, GlossaryEntry> {
    let mut m = HashMap::new();
    for e in entries {
        m.insert(e.source.clone(), e.clone());
    }
    m
}

fn write_glossary(path: &Path, entries: &[GlossaryEntry]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut raw = String::new();
    for e in entries {
        raw.push_str(&e.source);
        raw.push('\t');
        raw.push_str(&e.target);
        if !e.comment.is_empty() {
            raw.push('\t');
            raw.push_str(&e.comment);
        }
        raw.push('\n');
    }
    std::fs::write(path, raw)?;
    Ok(())
}

fn apply_tmx_resolution(props: &ProjectProperties, source: &str, translation: &str) -> Result<()> {
    let path = props.save_tmx_path();
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut tmx = parse_tmx(&raw, &props.source_lang, &props.target_lang);
    if let Some(e) = tmx.entries.iter_mut().find(|e| e.source == source) {
        e.translation = translation.to_string();
        e.note = None;
    }
    tmx.write(&path, &props.source_lang, &props.target_lang)
        .map_err(|e| TeamError::Command(e.to_string()))
}

fn apply_glossary_resolution(props: &ProjectProperties, source: &str, target: &str) -> Result<()> {
    let path = &props.glossary_file;
    if !path.exists() {
        return Ok(());
    }
    let mut entries = parse_glossary(&std::fs::read_to_string(path)?);
    if let Some(e) = entries.iter_mut().find(|e| e.source == source) {
        e.target = target.to_string();
    }
    write_glossary(path, &entries)
}

fn prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    match repo.repo_type.as_str() {
        "git" => git_prepare(props, repo),
        "svn" => svn_prepare(props, repo),
        "http" => http_prepare(props, repo),
        "file" => file_prepare(props, repo),
        other => Err(TeamError::Unsupported(other.into())),
    }
}

fn git_prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
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
            .unwrap_or_else(|| git_current_branch(&dir).unwrap_or_else(|_| "main".into()));
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
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_git(None, &args_ref)?;
    Ok(())
}

fn git_commit_and_push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    let dir = repo_work_dir(props, repo);
    if !dir.join(".git").exists() {
        return Ok(());
    }
    let _ = run_git(Some(&dir), &["add", "-A"]);
    let _ = git_commit(&dir, "OmegaT team sync");
    if is_inplace(props, repo) {
        let _ = run_git(Some(&dir), &["push"]);
        return Ok(());
    }
    let branch = repo
        .branch
        .clone()
        .unwrap_or_else(|| git_current_branch(&dir).unwrap_or_else(|_| "main".into()));
    let dest = format!("HEAD:refs/heads/{branch}");
    let _ = run_git(Some(&dir), &["push", "origin", &dest]);
    Ok(())
}

fn git_commit(dir: &Path, message: &str) -> Result<String> {
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

fn git_current_branch(dir: &Path) -> Result<String> {
    Ok(run_git(Some(dir), &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string())
}

fn svn_prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if !which("svn") {
        return Err(TeamError::Command(
            "svn client not installed (STATUS: SVN tests require svn + svnadmin)".into(),
        ));
    }
    if repo.url.is_empty() {
        return Ok(());
    }
    let dir = repo_work_dir(props, repo);
    if dir.join(".svn").exists() {
        run_cmd("svn", Some(&dir), &["update"])?;
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    run_cmd("svn", None, &["checkout", &repo.url, &dir.to_string_lossy()])?;
    Ok(())
}

fn svn_commit(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
    if !which("svn") {
        return Err(TeamError::Command("svn client not installed".into()));
    }
    let dir = repo_work_dir(props, repo);
    if !dir.join(".svn").exists() {
        return Ok(());
    }
    let _ = run_cmd("svn", Some(&dir), &["add", "--force", "."]);
    let _ = run_cmd("svn", Some(&dir), &["commit", "-m", "OmegaT team sync"]);
    Ok(())
}

fn http_prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
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

fn file_prepare(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
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

fn file_push(props: &ProjectProperties, repo: &RepositoryDef) -> Result<()> {
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

fn download(url: &str, dest: &Path) -> Result<()> {
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
    run_cmd(
        "curl",
        None,
        &["-fsSL", "-o", &dest.to_string_lossy(), url],
    )?;
    Ok(())
}

fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    Some(PathBuf::from(rest))
}

fn file_name_from_url(url: &str) -> String {
    url.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

#[derive(Clone, Copy)]
enum CopyDir {
    RepoToProject,
    ProjectToRepo,
}

fn copy_mapped(props: &ProjectProperties, repo: &RepositoryDef, dir: CopyDir) -> Result<()> {
    let wc = repo_work_dir(props, repo);
    if !wc.exists() {
        return Ok(());
    }
    for mapping in effective_mappings(repo) {
        let (from_root, from_rel, to_root, to_rel) = match dir {
            CopyDir::RepoToProject => (&wc, &mapping.repository, &props.root, &mapping.local),
            CopyDir::ProjectToRepo => (&props.root, &mapping.local, &wc, &mapping.repository),
        };
        let from = join_mapped(from_root, from_rel);
        let to = join_mapped(to_root, to_rel);
        if from.is_file() {
            if mapping_allows(&rel_unix(&from, from_root), &mapping)
                && !skip_copy(props, &rel_unix(&from, from_root), dir)
            {
                if let Some(p) = to.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(&from, &to)?;
            }
            continue;
        }
        if !from.is_dir() {
            continue;
        }
        for ent in walkdir::WalkDir::new(&from).into_iter().flatten() {
            if !ent.file_type().is_file() {
                continue;
            }
            let rel = ent
                .path()
                .strip_prefix(&from)
                .unwrap_or(ent.path())
                .to_string_lossy()
                .replace('\\', "/");
            let from_project_rel = join_rel(from_rel, &rel);
            if skip_copy(props, &from_project_rel, dir) {
                continue;
            }
            if !mapping_allows(&rel, &mapping) && !mapping_allows(&from_project_rel, &mapping) {
                continue;
            }
            let dest = to.join(&rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(ent.path(), dest)?;
        }
    }
    Ok(())
}

fn skip_copy(props: &ProjectProperties, rel: &str, dir: CopyDir) -> bool {
    let r = strip_slash(rel);
    if r.starts_with(".git/")
        || r == ".git"
        || r.starts_with(".svn/")
        || r == ".svn"
        || r.starts_with(".repositories/")
        || r == ".repositories"
    {
        return true;
    }
    if matches!(dir, CopyDir::ProjectToRepo) {
        return false;
    }
    if r == "omegat.project" || r.ends_with("project_save.tmx") || r.starts_with("target/") {
        return true;
    }
    let grel = props
        .glossary_file
        .strip_prefix(&props.root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if !grel.is_empty() && r == grel {
        return true;
    }
    false
}

fn mapping_allows(rel: &str, mapping: &RepositoryMapping) -> bool {
    let r = strip_slash(rel);
    for ex in &mapping.excludes {
        if glob_match(ex, r) {
            return false;
        }
    }
    if mapping.includes.is_empty() {
        return true;
    }
    mapping.includes.iter().any(|inc| glob_match(inc, r))
}

fn glob_match(pat: &str, path: &str) -> bool {
    let pat = strip_slash(pat);
    let path = strip_slash(path);
    if let Ok(g) = globset::Glob::new(pat) {
        if g.compile_matcher().is_match(path) {
            return true;
        }
    }
    if !pat.starts_with("**") {
        if let Ok(g) = globset::Glob::new(&format!("**/{pat}")) {
            if g.compile_matcher().is_match(path) {
                return true;
            }
        }
    }
    path == pat
}

fn effective_mappings(repo: &RepositoryDef) -> Vec<RepositoryMapping> {
    if repo.mappings.is_empty() {
        vec![default_mapping()]
    } else {
        repo.mappings.clone()
    }
}

fn default_mapping() -> RepositoryMapping {
    RepositoryMapping {
        local: "/".into(),
        repository: "/".into(),
        includes: vec![],
        excludes: vec![],
    }
}

fn join_mapped(base: &Path, mapped: &str) -> PathBuf {
    let rel = strip_slash(mapped);
    if rel.is_empty() {
        base.to_path_buf()
    } else {
        base.join(rel)
    }
}

fn join_rel(prefix: &str, rel: &str) -> String {
    let p = strip_slash(prefix);
    if p.is_empty() {
        rel.to_string()
    } else {
        format!("{p}/{rel}")
    }
}

fn rel_unix(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn strip_slash(s: &str) -> &str {
    s.trim_matches('/')
}

fn is_inplace(props: &ProjectProperties, repo: &RepositoryDef) -> bool {
    if repo.url.is_empty() {
        return true;
    }
    Path::new(&repo.url) == props.root || repo.url == props.root.to_string_lossy()
}

fn repo_work_dir(props: &ProjectProperties, repo: &RepositoryDef) -> PathBuf {
    if is_inplace(props, repo) {
        props.root.clone()
    } else {
        props
            .root
            .join(REPO_SUBDIR)
            .join(sanitize_url(&repo.url))
    }
}

fn sanitize_url(url: &str) -> String {
    let mut s = String::new();
    let mut prev_us = false;
    for c in url.chars() {
        if c.is_ascii_alphanumeric() || c == '.' {
            s.push(c);
            prev_us = false;
        } else if !prev_us {
            s.push('_');
            prev_us = true;
        }
    }
    if s.is_empty() {
        "repo".into()
    } else {
        s
    }
}

fn prep_dir(props: &ProjectProperties) -> PathBuf {
    props.root.join(REPO_SUBDIR).join(REPO_PREP)
}

fn conflicts_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("conflicts.json")
}

fn resolved_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("resolved.json")
}

fn base_tmx_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("base-project_save.tmx")
}

fn base_glossary_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("base-glossary.txt")
}

fn find_remote_tmx(props: &ProjectProperties) -> Option<PathBuf> {
    for repo in &props.repositories {
        let wc = repo_work_dir(props, repo);
        for cand in [
            wc.join("omegat").join("project_save.tmx"),
            wc.join("project_save.tmx"),
        ] {
            if cand.exists() {
                return Some(cand);
            }
        }
        if let Ok(rd) = std::fs::read_dir(&wc) {
            for ent in rd.flatten() {
                if ent.path().extension().and_then(|e| e.to_str()) == Some("tmx") {
                    return Some(ent.path());
                }
            }
        }
    }
    None
}

fn find_remote_glossary(props: &ProjectProperties) -> Option<PathBuf> {
    for repo in &props.repositories {
        let wc = repo_work_dir(props, repo);
        let rel = props
            .glossary_file
            .strip_prefix(&props.root)
            .ok()
            .map(|p| wc.join(p));
        for cand in [
            rel,
            Some(wc.join("glossary").join("glossary.txt")),
            Some(wc.join("glossary.txt")),
        ]
        .into_iter()
        .flatten()
        {
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

fn save_bases(props: &ProjectProperties) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    let tmx = props.save_tmx_path();
    if tmx.exists() {
        std::fs::copy(&tmx, base_tmx_path(props))?;
    }
    if props.glossary_file.exists() {
        std::fs::copy(&props.glossary_file, base_glossary_path(props))?;
    }
    Ok(())
}

fn save_conflicts(props: &ProjectProperties, conflicts: &[Conflict]) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    std::fs::write(conflicts_path(props), serde_json::to_string_pretty(conflicts).unwrap())?;
    Ok(())
}

fn read_resolved(props: &ProjectProperties) -> HashSet<String> {
    read_json::<Vec<String>>(&resolved_path(props))
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn mark_resolved(props: &ProjectProperties, source: &str) {
    let _ = std::fs::create_dir_all(prep_dir(props));
    let mut v: Vec<String> = read_json(&resolved_path(props)).unwrap_or_default();
    if !v.iter().any(|s| s == source) {
        v.push(source.into());
    }
    let _ = std::fs::write(resolved_path(props), serde_json::to_string(&v).unwrap());
}

fn clear_resolved(props: &ProjectProperties) {
    let _ = std::fs::remove_file(resolved_path(props));
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn copy_tree(from: &Path, to: &Path, skip_vcs: bool) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for ent in walkdir::WalkDir::new(from).into_iter().flatten() {
        if !ent.file_type().is_file() {
            continue;
        }
        let rel = ent.path().strip_prefix(from).unwrap_or(ent.path());
        let unix = rel.to_string_lossy().replace('\\', "/");
        if skip_vcs
            && (unix.starts_with(".git/")
                || unix == ".git"
                || unix.starts_with(".svn/")
                || unix.starts_with(".repositories/"))
        {
            continue;
        }
        let dest = to.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(ent.path(), dest)?;
    }
    Ok(())
}

fn run_git(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    run_cmd("git", dir, args)
}

fn run_cmd(bin: &str, dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut c = Command::new(bin);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    let out = c
        .args(args)
        .output()
        .map_err(|e| TeamError::Command(format!("{bin}: {e}")))?;
    if !out.status.success() {
        return Err(TeamError::Command(format!(
            "{bin} {} : {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn which(bin: &str) -> bool {
    Command::new(bin).arg("--version").output().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use omegat_core::properties::RepositoryMapping;
    use std::process::Command;

    fn tu(src: &str, tgt: &str) -> String {
        format!(
            r#"<tu><tuv lang="en"><seg>{src}</seg></tuv><tuv lang="fr"><seg>{tgt}</seg></tuv></tu>"#
        )
    }

    fn team_props(
        root: PathBuf,
        repo_type: &str,
        url: &str,
        mappings: Vec<RepositoryMapping>,
    ) -> ProjectProperties {
        let mut props = ProjectProperties::create(root, "en".into(), "fr".into(), false);
        props.repositories.push(RepositoryDef {
            repo_type: repo_type.into(),
            url: url.into(),
            branch: Some("main".into()),
            mappings,
        });
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        props
    }

    fn write_tmx(path: &Path, pairs: &[(&str, &str)]) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut raw = String::new();
        for (s, t) in pairs {
            raw.push_str(&tu(s, t));
        }
        std::fs::write(path, raw).unwrap();
    }

    #[test]
    fn rebase_keeps_ours_and_flags_conflict() {
        let ours = tu("Hi", "Salut");
        let theirs = tu("Hi", "Bonjour");
        let (tmx, conflicts) = rebase_tmx("", &ours, &theirs, "en", "fr");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
        assert!(tmx
            .get("Hi")
            .unwrap()
            .note
            .as_ref()
            .unwrap()
            .contains("Bonjour"));
    }

    #[test]
    fn file_sync_copies_and_rebases() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("omegat")).unwrap();
        write_tmx(&remote.join("omegat").join("project_save.tmx"), &[("Hi", "Bonjour")]);
        let props = team_props(
            local.clone(),
            "file",
            &remote.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].ours, "Salut");
        assert_eq!(c[0].theirs, "Bonjour");
        assert_eq!(c[0].kind, "tmx");
    }

    #[test]
    fn file_sync_merges_different_segments_and_glossary() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("omegat")).unwrap();
        std::fs::create_dir_all(remote.join("glossary")).unwrap();
        write_tmx(&remote.join("omegat").join("project_save.tmx"), &[("Hi", "Bonjour")]);
        std::fs::write(remote.join("glossary").join("glossary.txt"), "cat\tchat\n").unwrap();
        let props = team_props(
            local.clone(),
            "file",
            &remote.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Bye", "Au revoir")]);
        std::fs::write(&props.glossary_file, "dog\tchien\n").unwrap();
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "sync");
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Bonjour");
        assert_eq!(tmx.get("Bye").unwrap().translation, "Au revoir");
        let gloss = std::fs::read_to_string(&props.glossary_file).unwrap();
        assert!(gloss.contains("cat\tchat"));
        assert!(gloss.contains("dog\tchien"));
    }

    #[test]
    fn glossary_conflict_is_structured_and_resolvable() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("glossary")).unwrap();
        std::fs::write(remote.join("glossary").join("glossary.txt"), "cat\tchat\n").unwrap();
        write_tmx(&remote.join("omegat").join("project_save.tmx"), &[]);
        let props = team_props(local, "file", &remote.to_string_lossy(), vec![default_mapping()]);
        std::fs::write(&props.glossary_file, "cat\tfelin\n").unwrap();
        write_tmx(&props.save_tmx_path(), &[]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let left = resolve(&props, "cat", "theirs", None).unwrap();
        assert!(left.is_empty());
        let gloss = std::fs::read_to_string(&props.glossary_file).unwrap();
        assert!(gloss.contains("cat\tchat"));
    }

    #[test]
    fn http_downloads_remote_tmx_into_rebase() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("mem.tmx");
        write_tmx(&remote, &[("Hi", "Bonjour")]);
        let local = dir.path().join("local");
        let url = format!("file://{}", remote.display());
        let props = team_props(
            local,
            "http",
            &url,
            vec![RepositoryMapping {
                local: "omegat/project_save.tmx".into(),
                repository: "project_save.tmx".into(),
                includes: vec![],
                excludes: vec![],
            }],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let err = sync(&props).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props);
        assert_eq!(c[0].theirs, "Bonjour");
        let left = resolve(&props, "Hi", "ours", None).unwrap();
        assert!(left.is_empty());
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
    }

    #[test]
    fn mapping_excludes_are_applied() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote");
        let local = dir.path().join("local");
        std::fs::create_dir_all(remote.join("source")).unwrap();
        std::fs::write(remote.join("source").join("keep.txt"), "keep").unwrap();
        std::fs::write(remote.join("source").join("skip.bak"), "skip").unwrap();
        let props = team_props(
            local,
            "file",
            &remote.to_string_lossy(),
            vec![RepositoryMapping {
                local: "/".into(),
                repository: "/".into(),
                includes: vec![],
                excludes: vec!["**/*.bak".into()],
            }],
        );
        write_tmx(&props.save_tmx_path(), &[]);
        sync(&props).unwrap();
        assert!(props.source_dir.join("keep.txt").exists());
        assert!(!props.source_dir.join("skip.bak").exists());
    }

    #[test]
    fn empty_repository_list_is_local() {
        let dir = tempfile::tempdir().unwrap();
        let props = ProjectProperties::create(dir.path().to_path_buf(), "en".into(), "fr".into(), false);
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "local");
    }

    fn seed_bare(bare: &Path, seed: &Path) {
        assert!(Command::new("git")
            .args(["init", "--bare", &bare.to_string_lossy()])
            .status()
            .unwrap()
            .success());
        std::fs::create_dir_all(seed.join("omegat")).unwrap();
        write_tmx(&seed.join("omegat").join("project_save.tmx"), &[]);
        std::fs::create_dir_all(seed.join("glossary")).unwrap();
        std::fs::write(seed.join("glossary").join("glossary.txt"), "").unwrap();
        assert!(Command::new("git")
            .args(["init"])
            .current_dir(seed)
            .status()
            .unwrap()
            .success());
        let _ = Command::new("git")
            .args(["checkout", "-B", "main"])
            .current_dir(seed)
            .status();
        let _ = run_git(Some(seed), &["add", "-A"]);
        git_commit(seed, "seed").unwrap();
        run_git(
            Some(seed),
            &["remote", "add", "origin", &bare.to_string_lossy()],
        )
        .unwrap();
        run_git(Some(seed), &["push", "-u", "origin", "HEAD:refs/heads/main"]).unwrap();
    }

    #[test]
    fn git_two_clients_merge_different_segments() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for R6 two-client test");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let props_a = team_props(
            a,
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_a.save_tmx_path(), &[("Hi", "Salut")]);
        let r = sync(&props_a).unwrap();
        assert_eq!(r.action, "sync");

        let props_b = team_props(
            b,
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_b.save_tmx_path(), &[("Bye", "Au revoir")]);
        sync(&props_b).unwrap();
        let tmx_b = parse_tmx(
            &std::fs::read_to_string(props_b.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx_b.get("Hi").unwrap().translation, "Salut");
        assert_eq!(tmx_b.get("Bye").unwrap().translation, "Au revoir");

        sync(&props_a).unwrap();
        let tmx_a = parse_tmx(
            &std::fs::read_to_string(props_a.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx_a.get("Hi").unwrap().translation, "Salut");
        assert_eq!(tmx_a.get("Bye").unwrap().translation, "Au revoir");
    }

    #[test]
    fn git_two_clients_same_segment_conflicts_then_resolve() {
        if Command::new("git").arg("--version").output().is_err() {
            panic!("git is required for R6 conflict test");
        }
        let dir = tempfile::tempdir().unwrap();
        let bare = dir.path().join("remote.git");
        seed_bare(&bare, &dir.path().join("seed"));
        let props_a = team_props(
            dir.path().join("a"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_a.save_tmx_path(), &[("Hi", "Salut")]);
        sync(&props_a).unwrap();

        let props_b = team_props(
            dir.path().join("b"),
            "git",
            &bare.to_string_lossy(),
            vec![default_mapping()],
        );
        write_tmx(&props_b.save_tmx_path(), &[("Hi", "Bonjour")]);
        let err = sync(&props_b).unwrap_err();
        assert!(matches!(err, TeamError::Conflict(_)));
        let c = list_conflicts(&props_b);
        assert_eq!(c[0].ours, "Bonjour");
        assert_eq!(c[0].theirs, "Salut");
        resolve(&props_b, "Hi", "theirs", None).unwrap();
        let tmx = parse_tmx(
            &std::fs::read_to_string(props_b.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
        sync(&props_b).unwrap();
    }

    #[test]
    fn svn_checkout_update_commit() {
        if !which("svn") || !which("svnadmin") {
            eprintln!("skip svn_checkout_update_commit: svn/svnadmin not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("svnrepo");
        run_cmd("svnadmin", None, &["create", &repo.to_string_lossy()]).unwrap();
        let url = format!("file://{}", repo.display());
        let seed = dir.path().join("seed");
        run_cmd("svn", None, &["checkout", &url, &seed.to_string_lossy()]).unwrap();
        std::fs::create_dir_all(seed.join("omegat")).unwrap();
        write_tmx(&seed.join("omegat").join("project_save.tmx"), &[]);
        let _ = run_cmd("svn", Some(&seed), &["add", "omegat"]);
        run_cmd("svn", Some(&seed), &["commit", "-m", "seed"]).unwrap();

        let props = team_props(
            dir.path().join("client"),
            "svn",
            &url,
            vec![default_mapping()],
        );
        write_tmx(&props.save_tmx_path(), &[("Hi", "Salut")]);
        let r = sync(&props).unwrap();
        assert_eq!(r.action, "sync");
        let tmx = parse_tmx(
            &std::fs::read_to_string(props.save_tmx_path()).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(tmx.get("Hi").unwrap().translation, "Salut");
    }
}
