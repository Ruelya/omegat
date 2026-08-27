//! Java `RebaseUtils`.

use crate::error::Result;
use crate::project_team_settings::{base_glossary_path, base_tmx_path, prep_dir, repo_work_dir};
use omegat_core::properties::ProjectProperties;
use std::path::PathBuf;

pub fn find_remote_tmx(props: &ProjectProperties) -> Option<PathBuf> {
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

pub fn find_remote_glossary(props: &ProjectProperties) -> Option<PathBuf> {
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

pub fn save_bases(props: &ProjectProperties) -> Result<()> {
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
