//! Mapping include/exclude used by `RemoteRepositoryProvider`.

use crate::error::Result;
use crate::project_team_settings::repo_work_dir;
use crate::team_utils::{join_mapped, join_rel, rel_unix, strip_slash};
use omegat_core::properties::{ProjectProperties, RepositoryDef, RepositoryMapping};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyDir {
    RepoToProject,
    ProjectToRepo,
}

pub fn default_mapping() -> RepositoryMapping {
    RepositoryMapping {
        local: "/".into(),
        repository: "/".into(),
        includes: vec![],
        excludes: vec![],
    }
}

pub fn effective_mappings(repo: &RepositoryDef) -> Vec<RepositoryMapping> {
    if repo.mappings.is_empty() {
        vec![default_mapping()]
    } else {
        repo.mappings.clone()
    }
}

pub fn file_name_from_url(url: &str) -> String {
    url.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

pub fn mapping_allows(rel: &str, mapping: &RepositoryMapping) -> bool {
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

pub fn glob_match(pat: &str, path: &str) -> bool {
    let anchored = pat.starts_with('/') || pat.starts_with('\\');
    let pat = pat.trim_matches(['/', '\\']);
    let path = strip_slash(path);
    let matches = |pattern: &str| {
        globset::GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .is_ok_and(|glob| glob.compile_matcher().is_match(path))
    };
    if matches(pat) {
        return true;
    }
    if !anchored && !pat.starts_with("**") && matches(&format!("**/{pat}")) {
        return true;
    }
    path == pat
}

pub fn skip_copy(props: &ProjectProperties, rel: &str, dir: CopyDir) -> bool {
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

pub fn copy_mapped(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    dir: CopyDir,
) -> Result<Vec<String>> {
    let wc = repo_work_dir(props, repo);
    copy_mapped_from_worktree(props, repo, &wc, dir)
}

/// Execute repository mappings against an already prepared worktree.
///
/// The returned paths are relative to the destination root. Keeping the
/// operation observable makes the same product path useful to sync/commit and
/// to callers that need Java `RemoteRepositoryProvider`-style copy reporting.
pub fn copy_mapped_from_worktree(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    wc: &std::path::Path,
    dir: CopyDir,
) -> Result<Vec<String>> {
    let mut copied = Vec::new();
    if !wc.exists() {
        return Ok(copied);
    }
    for mapping in effective_mappings(repo) {
        let (from_root, from_rel, to_root, to_rel) = match dir {
            CopyDir::RepoToProject => (
                wc,
                &mapping.repository,
                props.root.as_path(),
                &mapping.local,
            ),
            CopyDir::ProjectToRepo => (
                props.root.as_path(),
                &mapping.local,
                wc,
                &mapping.repository,
            ),
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
                copied.push(rel_unix(&to, to_root));
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
            std::fs::copy(ent.path(), &dest)?;
            copied.push(rel_unix(&dest, to_root));
        }
    }
    Ok(copied)
}
