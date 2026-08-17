//! Java `TeamTool`.

use crate::error::{Result, TeamError};
use crate::git_remote_repository2;
use crate::mapping::default_mapping;
use crate::team_utils::run_git;
use omegat_core::properties::{ProjectProperties, RepositoryDef};
use std::path::Path;

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
        let _ = git_remote_repository2::commit(dir, "OmegaT team init");
    }
    Ok(())
}
