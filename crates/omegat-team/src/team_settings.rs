//! Java `TeamSettings` — persisted conflict / resolved lists under `.repositories/prep/`.

use crate::error::{Conflict, Result};
use crate::project_team_settings::{conflicts_path, prep_dir, resolved_path};
use crate::team_utils::read_json;
use omegat_core::properties::ProjectProperties;
use std::collections::HashSet;

pub fn list_conflicts(props: &ProjectProperties) -> Vec<Conflict> {
    read_json(&conflicts_path(props)).unwrap_or_default()
}

pub fn save_conflicts(props: &ProjectProperties, conflicts: &[Conflict]) -> Result<()> {
    std::fs::create_dir_all(prep_dir(props))?;
    std::fs::write(
        conflicts_path(props),
        serde_json::to_string_pretty(conflicts).unwrap(),
    )?;
    Ok(())
}

pub fn read_resolved(props: &ProjectProperties) -> HashSet<String> {
    read_json::<Vec<String>>(&resolved_path(props))
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub fn mark_resolved(props: &ProjectProperties, source: &str) {
    let _ = std::fs::create_dir_all(prep_dir(props));
    let mut v: Vec<String> = read_json(&resolved_path(props)).unwrap_or_default();
    if !v.iter().any(|s| s == source) {
        v.push(source.into());
    }
    let _ = std::fs::write(resolved_path(props), serde_json::to_string(&v).unwrap());
}

pub fn clear_resolved(props: &ProjectProperties) {
    let _ = std::fs::remove_file(resolved_path(props));
}
