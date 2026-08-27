//! Java `GlossaryRebaseOperation`.

use crate::error::{Conflict, Result};
use crate::i_rebase_operation::IRebaseOperation;
use crate::project_team_settings::base_glossary_path;
use crate::rebase_utils::find_remote_glossary;
use omegat_core::glossary::{parse_glossary, GlossaryEntry};
use omegat_core::properties::ProjectProperties;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct GlossaryRebaseOperation;

impl IRebaseOperation for GlossaryRebaseOperation {
    fn rebase(
        &self,
        props: &ProjectProperties,
        resolved: &HashSet<String>,
    ) -> Result<Vec<Conflict>> {
        rebase_files(props, resolved)
    }
}

pub fn rebase_files(props: &ProjectProperties, resolved: &HashSet<String>) -> Result<Vec<Conflict>> {
    let ours_path = &props.glossary_file;
    let Some(theirs_path) = find_remote_glossary(props) else {
        return Ok(vec![]);
    };
    if !ours_path.exists() {
        if let Some(parent) = ours_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&theirs_path, ours_path)?;
        return Ok(vec![]);
    }
    let ours = parse_glossary(&std::fs::read_to_string(ours_path)?);
    let theirs = parse_glossary(&std::fs::read_to_string(&theirs_path)?);
    let base = parse_glossary(&std::fs::read_to_string(base_glossary_path(props)).unwrap_or_default());
    let (merged, conflicts) = rebase(&base, &ours, &theirs, resolved);
    write_glossary(ours_path, &merged)?;
    Ok(conflicts)
}

pub fn rebase(
    base: &[GlossaryEntry],
    ours: &[GlossaryEntry],
    theirs: &[GlossaryEntry],
    resolved: &HashSet<String>,
) -> (Vec<GlossaryEntry>, Vec<Conflict>) {
    let b = glossary_map(base);
    let o = glossary_map(ours);
    let t = glossary_map(theirs);
    let mut keys = HashSet::new();
    keys.extend(b.keys().cloned());
    keys.extend(o.keys().cloned());
    keys.extend(t.keys().cloned());
    let mut out = Vec::new();
    let mut conflicts = Vec::new();
    for k in keys {
        match (o.get(&k), t.get(&k)) {
            (Some(a), Some(tb)) if a.target != tb.target => {
                if resolved.contains(&k) {
                    out.push(a.clone());
                    continue;
                }
                let base_t = b.get(&k).map(|e| e.target.as_str()).unwrap_or("");
                if !base_t.is_empty() && a.target == base_t {
                    out.push(tb.clone());
                } else if !base_t.is_empty() && tb.target == base_t {
                    out.push(a.clone());
                } else {
                    conflicts.push(Conflict {
                        kind: "glossary".into(),
                        source: k,
                        ours: a.target.clone(),
                        theirs: tb.target.clone(),
                        message: format!("glossary conflict on {}", a.source),
                    });
                    out.push(a.clone());
                }
            }
            (Some(a), _) => out.push(a.clone()),
            (None, Some(tb)) => out.push(tb.clone()),
            (None, None) => {
                if let Some(be) = b.get(&k) {
                    out.push(be.clone());
                }
            }
        }
    }
    out.sort_by(|a, b| a.source.cmp(&b.source));
    (out, conflicts)
}

fn glossary_map(entries: &[GlossaryEntry]) -> HashMap<String, GlossaryEntry> {
    let mut m = HashMap::new();
    for e in entries {
        m.insert(e.source.clone(), e.clone());
    }
    m
}

pub fn write_glossary(path: &Path, entries: &[GlossaryEntry]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut raw = String::new();
    for e in entries {
        raw.push_str(&e.source);
        raw.push('\t');
        raw.push_str(&e.target);
        if !e.comment.is_empty() {
            raw.push('\t');
            raw.push_str(&e.comment);
        }
        raw.push('\n');
    }
    std::fs::write(path, raw)?;
    Ok(())
}

pub fn apply_resolution(props: &ProjectProperties, source: &str, target: &str) -> Result<()> {
    let path = &props.glossary_file;
    if !path.exists() {
        return Ok(());
    }
    let mut entries = parse_glossary(&std::fs::read_to_string(path)?);
    if let Some(e) = entries.iter_mut().find(|e| e.source == source) {
        e.target = target.to_string();
    }
    write_glossary(path, &entries)
}
