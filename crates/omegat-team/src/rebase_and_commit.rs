//! Java `RebaseAndCommit`.

use crate::error::{Conflict, Result};
use crate::glossary_rebase::{self, GlossaryRebaseOperation};
use crate::i_rebase_operation::IRebaseOperation;
use crate::team_settings::{list_conflicts, mark_resolved, read_resolved, save_conflicts};
use crate::tmx_rebase::{self, TMXRebaseOperation};
use omegat_core::properties::ProjectProperties;

pub fn rebase_all(props: &ProjectProperties) -> Result<Vec<Conflict>> {
    let resolved = read_resolved(props);
    let mut conflicts = TMXRebaseOperation.rebase(props, &resolved)?;
    conflicts.extend(GlossaryRebaseOperation.rebase(props, &resolved)?);
    Ok(conflicts)
}

pub fn rebase_project(props: &ProjectProperties) -> Result<Vec<String>> {
    let conflicts = rebase_all(props)?;
    save_conflicts(props, &conflicts)?;
    Ok(conflicts.into_iter().map(|c| c.source).collect())
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
        glossary_rebase::apply_resolution(props, source, &chosen)?;
    } else {
        tmx_rebase::apply_resolution(props, source, &chosen)?;
    }
    mark_resolved(props, source);
    conflicts.remove(idx);
    save_conflicts(props, &conflicts)?;
    Ok(conflicts)
}
