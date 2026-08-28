//! Java `TMXRebaseOperation`.

use crate::error::{Conflict, Result, TeamError};
use crate::i_rebase_operation::IRebaseOperation;
use crate::project_team_settings::base_tmx_path;
use crate::rebase_utils::find_remote_tmx;
use omegat_core::properties::ProjectProperties;
use omegat_core::tmx::{parse_tmx, ProjectTmx, TmxEntry};
use omegat_ipc::EntryKeyDto;
use std::collections::{BTreeMap, BTreeSet, HashSet};

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
    let b = entries_by_identity(b.entries);
    let o = entries_by_identity(o.entries);
    let t = entries_by_identity(t.entries);
    let mut out = ProjectTmx::new();
    let mut conflicts = Vec::new();
    let keys: BTreeSet<TmxIdentity> = b.keys().chain(o.keys()).chain(t.keys()).cloned().collect();
    for key in keys {
        let ov = o.get(&key);
        let tv = t.get(&key);
        let bv = b.get(&key);
        match (ov, tv) {
            (Some(a), Some(tb)) if a.translation != tb.translation => {
                if resolved.contains(&key.resolution_id()) {
                    out.insert(a.clone());
                    continue;
                }
                let base_t = bv.map(|e| e.translation.as_str()).unwrap_or("");
                if !base_t.is_empty() && a.translation == base_t {
                    out.insert(tb.clone());
                } else if !base_t.is_empty() && tb.translation == base_t {
                    out.insert(a.clone());
                } else {
                    let source = key.source().to_string();
                    conflicts.push(Conflict {
                        kind: "tmx".into(),
                        source: source.clone(),
                        ours: a.translation.clone(),
                        theirs: tb.translation.clone(),
                        message: format!("TMX conflict on {source}"),
                        entry_key: key.entry_key().cloned(),
                    });
                    out.insert(TmxEntry {
                        source,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TmxIdentity {
    Default(String),
    Entry(EntryKeyDto),
}

impl TmxIdentity {
    fn from_entry(entry: &TmxEntry) -> Self {
        match (entry.default_translation, entry.file.as_ref()) {
            (false, Some(file)) => Self::Entry(EntryKeyDto {
                file: file.clone(),
                source_text: entry.source.clone(),
                id: entry.id.clone(),
                prev: entry.prev.clone(),
                next: entry.next.clone(),
                path: entry.path.clone(),
            }),
            _ => Self::Default(entry.source.clone()),
        }
    }

    fn source(&self) -> &str {
        match self {
            Self::Default(source) => source,
            Self::Entry(key) => &key.source_text,
        }
    }

    fn entry_key(&self) -> Option<&EntryKeyDto> {
        match self {
            Self::Default(_) => None,
            Self::Entry(key) => Some(key),
        }
    }

    fn resolution_id(&self) -> String {
        conflict_resolution_id(self.source(), self.entry_key())
    }
}

fn entries_by_identity(entries: Vec<TmxEntry>) -> BTreeMap<TmxIdentity, TmxEntry> {
    entries
        .into_iter()
        .map(|entry| (TmxIdentity::from_entry(&entry), entry))
        .collect()
}

pub(crate) fn conflict_resolution_id(source: &str, entry_key: Option<&EntryKeyDto>) -> String {
    entry_key.map_or_else(
        || source.to_string(),
        |key| {
            format!(
                "entry-key:{}",
                serde_json::to_string(key).expect("EntryKey JSON serialization")
            )
        },
    )
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
    apply_resolution_for_key(props, source, None, translation)
}

pub fn apply_resolution_for_key(
    props: &ProjectProperties,
    source: &str,
    entry_key: Option<&EntryKeyDto>,
    translation: &str,
) -> Result<()> {
    let path = props.save_tmx_path();
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut tmx = parse_tmx(&raw, &props.source_lang, &props.target_lang);
    let entry = tmx.entries.iter_mut().find(|entry| {
        if let Some(key) = entry_key {
            TmxIdentity::from_entry(entry) == TmxIdentity::Entry(key.clone())
        } else {
            entry.source == source && entry.default_translation
        }
    });
    if let Some(e) = entry {
        e.translation = translation.to_string();
        e.note = None;
    } else {
        return Err(TeamError::Conflict(format!(
            "TMX conflict entry is no longer available: {}",
            conflict_resolution_id(source, entry_key)
        )));
    }
    tmx.write(&path, &props.source_lang, &props.target_lang)
        .map_err(|e| TeamError::Command(e.to_string()))
}
