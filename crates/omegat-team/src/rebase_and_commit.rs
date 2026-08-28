//! Java `RebaseAndCommit`.

use crate::error::{Conflict, Result};
use crate::glossary_rebase::{self, GlossaryRebaseOperation};
use crate::i_rebase_operation::IRebaseOperation;
use crate::team_settings::{
    list_conflicts, mark_resolved, read_resolved, save_conflicts, save_conflicts_cancellable,
};
use crate::tmx_rebase::{self, TMXRebaseOperation};
use omegat_core::cancellation::CancellationToken;
use omegat_core::properties::ProjectProperties;
use omegat_ipc::EntryKeyDto;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static CRASH_AFTER_RESOLUTION_WRITEBACK: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn crash_after_resolution_writeback() {
    CRASH_AFTER_RESOLUTION_WRITEBACK.store(true, Ordering::SeqCst);
}

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
    resolve_for_key(props, source, None, side, translation)
}

pub fn resolve_for_key(
    props: &ProjectProperties,
    source: &str,
    entry_key: Option<&EntryKeyDto>,
    side: &str,
    translation: Option<&str>,
) -> Result<Vec<Conflict>> {
    resolve_for_key_cancellable(
        props,
        source,
        entry_key,
        side,
        translation,
        &CancellationToken::default(),
    )
}

pub fn resolve_for_key_cancellable(
    props: &ProjectProperties,
    source: &str,
    entry_key: Option<&EntryKeyDto>,
    side: &str,
    translation: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<Vec<Conflict>> {
    resolve_for_key_cancellable_scoped(
        props,
        source,
        entry_key,
        side,
        translation,
        cancellation,
        0,
        None,
    )
}

pub fn resolve_for_key_cancellable_scoped(
    props: &ProjectProperties,
    source: &str,
    entry_key: Option<&EntryKeyDto>,
    side: &str,
    translation: Option<&str>,
    cancellation: &CancellationToken,
    generation: u64,
    batch_id: Option<&str>,
) -> Result<Vec<Conflict>> {
    crate::remote_repository_provider::commit_product_transaction_cancellable(
        props,
        "resolve-conflict",
        cancellation,
        "team.resolve.snapshot",
        generation,
        batch_id,
        |cancellation| {
            if cancellation.is_cancelled() {
                return Err(crate::TeamError::Cancelled);
            }
            let mut conflicts = list_conflicts(props);
            let idx = entry_key
                .and_then(|key| {
                    conflicts
                        .iter()
                        .position(|conflict| conflict.entry_key.as_ref() == Some(key))
                })
                .or_else(|| {
                    conflicts.iter().position(|conflict| {
                        conflict.source == source
                            && (entry_key.is_none() || conflict.entry_key.is_none())
                    })
                });
            let Some(idx) = idx else {
                return Ok(conflicts);
            };
            let chosen = match side {
                "theirs" => conflicts[idx].theirs.clone(),
                "manual" => translation.unwrap_or(&conflicts[idx].ours).to_string(),
                _ => conflicts[idx].ours.clone(),
            };
            let kind = conflicts[idx].kind.clone();
            let conflict_key = conflicts[idx].entry_key.clone();
            if kind == "glossary" {
                glossary_rebase::apply_resolution(props, source, &chosen)?;
            } else {
                tmx_rebase::apply_resolution_for_key(
                    props,
                    source,
                    conflict_key.as_ref(),
                    &chosen,
                )?;
            }
            #[cfg(test)]
            if CRASH_AFTER_RESOLUTION_WRITEBACK.swap(false, Ordering::SeqCst) {
                std::process::abort();
            }
            if cancellation.checkpoint("team.resolve.writeback") {
                return Err(crate::TeamError::Cancelled);
            }
            mark_resolved(
                props,
                &tmx_rebase::conflict_resolution_id(source, conflict_key.as_ref()),
            )?;
            if cancellation.checkpoint("team.resolve.resolved") {
                return Err(crate::TeamError::Cancelled);
            }
            conflicts.remove(idx);
            save_conflicts_cancellable(props, &conflicts, cancellation)?;
            if cancellation.checkpoint("team.resolve.conflicts") {
                return Err(crate::TeamError::Cancelled);
            }
            Ok(conflicts)
        },
    )
}
