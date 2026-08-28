//! Java `ProjectTeamSettings`.

use crate::team_utils::sanitize_url;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const REPO_SUBDIR: &str = ".repositories";
pub const REPO_PREP: &str = "prep";

pub fn is_inplace(props: &ProjectProperties, repo: &RepositoryDef) -> bool {
    if repo.url.is_empty() {
        return true;
    }
    Path::new(&repo.url) == props.root || repo.url == props.root.to_string_lossy()
}

pub fn repo_work_dir(props: &ProjectProperties, repo: &RepositoryDef) -> PathBuf {
    if is_inplace(props, repo) {
        props.root.clone()
    } else {
        props.root.join(REPO_SUBDIR).join(sanitize_url(&repo.url))
    }
}

pub fn prep_dir(props: &ProjectProperties) -> PathBuf {
    props.root.join(REPO_SUBDIR).join(REPO_PREP)
}

pub fn conflicts_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("conflicts.json")
}

pub fn resolved_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("resolved.json")
}

pub fn base_tmx_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("base-project_save.tmx")
}

pub fn base_glossary_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("base-glossary.txt")
}

pub fn credentials_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("credentials.json")
}

fn repository_state_path(props: &ProjectProperties) -> PathBuf {
    prep_dir(props).join("repository-state.json")
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RepositoryState {
    last_delete_check: HashMap<String, String>,
}

fn state_key(props: &ProjectProperties, repo: &RepositoryDef) -> String {
    if is_inplace(props, repo) {
        "inplace".into()
    } else {
        sanitize_url(&repo.url)
    }
}

pub fn last_delete_check(
    props: &ProjectProperties,
    repo: &RepositoryDef,
) -> crate::Result<Option<String>> {
    let path = repository_state_path(props);
    if !path.is_file() {
        return Ok(None);
    }
    let state: RepositoryState = serde_json::from_str(&std::fs::read_to_string(path)?)
        .map_err(|error| crate::error::TeamError::Command(format!("repository state: {error}")))?;
    Ok(state
        .last_delete_check
        .get(&state_key(props, repo))
        .cloned())
}

pub fn set_last_delete_check(
    props: &ProjectProperties,
    repo: &RepositoryDef,
    version: &str,
) -> crate::Result<()> {
    let path = repository_state_path(props);
    let mut state = if path.is_file() {
        serde_json::from_str(&std::fs::read_to_string(&path)?).map_err(|error| {
            crate::error::TeamError::Command(format!("repository state: {error}"))
        })?
    } else {
        RepositoryState::default()
    };
    state
        .last_delete_check
        .insert(state_key(props, repo), version.to_string());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&state)
        .map_err(|error| crate::error::TeamError::Command(format!("repository state: {error}")))?;
    std::fs::write(path, json)?;
    Ok(())
}
