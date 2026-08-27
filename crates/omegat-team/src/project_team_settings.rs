//! Java `ProjectTeamSettings`.

use crate::team_utils::sanitize_url;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
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
