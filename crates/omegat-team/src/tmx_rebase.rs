//! Java `TMXRebaseOperation`.

use crate::error::{Conflict, Result, TeamError};
use crate::i_rebase_operation::IRebaseOperation;
use crate::project_team_settings::base_tmx_path;
use crate::rebase_utils::find_remote_tmx;
use omegat_core::properties::ProjectProperties;
use omegat_core::tmx::{parse_tmx, ProjectTmx, TmxEntry};
use std::collections::HashSet;

pub struct TMXRebaseOperation;

impl IRebaseOperation for TMXRebaseOperation {
    fn rebase(
        &self,
        props: &ProjectProperties,
        resolved: &HashSet<String>,
    ) -> Result<Vec<Conflict>> {
        rebase_files(props, resolved)
    }
}

/// Three-way TMX merge: base / ours / theirs. Conflicting sources keep ours and record theirs.
pub fn rebase_tmx(
    base: &str,
    ours: &str,
    theirs: &str,
    sl: &str,
    tl: &str,
) -> (ProjectTmx, Vec<String>) {
    let (tmx, conflicts) = rebase_detailed(base, ours, theirs, sl, tl, &HashSet::new());
    (tmx, conflicts.into_iter().map(|c| c.source).collect())
}

pub fn rebase_detailed(
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
    for e in b
        .entries
        .iter()
        .chain(o.entries.iter())
        .chain(t.entries.iter())
    {
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

pub fn rebase_files(
    props: &ProjectProperties,
    resolved: &HashSet<String>,
) -> Result<Vec<Conflict>> {
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
    let (merged, conflicts) = rebase_detailed(
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

pub fn apply_resolution(props: &ProjectProperties, source: &str, translation: &str) -> Result<()> {
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
