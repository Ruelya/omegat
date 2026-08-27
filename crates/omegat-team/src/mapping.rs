//! Mapping include/exclude used by `RemoteRepositoryProvider`.

use crate::error::Result;
use crate::project_team_settings::repo_work_dir;
use crate::team_utils::{join_mapped, join_rel, rel_unix, strip_slash};
use omegat_core::properties::{ProjectProperties, RepositoryDef, RepositoryMapping};

#[derive(Clone, Copy)]
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

pub fn copy_mapped(props: &ProjectProperties, repo: &RepositoryDef, dir: CopyDir) -> Result<()> {
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
