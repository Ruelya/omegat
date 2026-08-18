//! External TM folders: `tm/auto`, `tm/enforce`, `tm/mt`, `tm/penalty-NNN`, `tmx2source`.
//! Java `ExternalTMFactory`.

use crate::consts::*;
use crate::properties::ProjectProperties;
use crate::tmx::{self, ProjectTmx, TmxEntry};
use omegat_filters::{FilterContext, FilterRegistry};
use std::path::Path;

pub fn penalty_from_origin(origin: &str) -> i32 {
    origin
        .replace('\\', "/")
        .split('/')
        .find_map(|p| p.strip_prefix("penalty-")?.parse().ok())
        .unwrap_or(0)
}

pub fn folder_is(origin: &str, name: &str) -> bool {
    let o = origin.replace('\\', "/");
    o == name || o.starts_with(&format!("{name}/")) || o.contains(&format!("/{name}/"))
}

pub fn load_external_tm(props: &ProjectProperties) -> Vec<(TmxEntry, String)> {
    let mut out = Vec::new();
    if !props.tm_dir.exists() {
        return out;
    }
    for ent in walkdir::WalkDir::new(&props.tm_dir).into_iter().flatten() {
        if ent.path().extension().and_then(|e| e.to_str()) != Some("tmx") {
            continue;
        }
        let origin = ent
            .path()
            .strip_prefix(&props.tm_dir)
            .unwrap_or(ent.path())
            .to_string_lossy()
            .replace('\\', "/");
        let langs = if folder_is(&origin, TMX2SOURCE) {
            let stem = ent
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&props.target_lang);
            (props.source_lang.as_str(), stem)
        } else {
            (props.source_lang.as_str(), props.target_lang.as_str())
        };
        if let Ok(tmx) = ProjectTmx::load(ent.path(), langs.0, langs.1) {
            let penalty = penalty_from_origin(&origin);
            for mut e in tmx.entries {
                e.penalty = penalty;
                if penalty > 0 {
                    e.note = Some(format!("penalty:{penalty}"));
                }
                out.push((e, origin.clone()));
            }
        }
    }
    out
}

pub fn is_supported(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.ends_with(".tmx")
        || name.ends_with(".po")
        || name.ends_with(".lang")
        || name.ends_with(".xlf")
        || name.ends_with(".xliff")
}

pub fn load(path: &Path, source_lang: &str, target_lang: &str, keep_foreign: bool) -> Vec<TmxEntry> {
    if !path.exists() {
        return vec![];
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tmx") {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return vec![];
        };
        let loaded = tmx::parse_external_tmx(&raw, source_lang, target_lang, keep_foreign);
        return resegment(loaded, source_lang, target_lang);
    }
    let ctx = FilterContext {
        source_lang: source_lang.into(),
        target_lang: target_lang.into(),
        ..Default::default()
    };
    let reg = FilterRegistry::new();
    let Some(filter) = reg.for_path(path) else {
        return vec![];
    };
    let Ok(parsed) = filter.parse(path, &ctx) else {
        return vec![];
    };
    let loaded: Vec<TmxEntry> = parsed
        .segments
        .into_iter()
        .filter_map(|s| {
            // Java BifileLoader: skip when source or translation is null.
            // POFilter sets empty msgstr to null.
            let translation = s.existing_translation?;
            let source = strip_some_chars(&s.source, true);
            let translation = strip_some_chars(&translation, true);
            if source.trim().is_empty() {
                return None;
            }
            Some(TmxEntry {
                source,
                translation,
                note: s.note.or(s.comment),
                default_translation: true,
                ..Default::default()
            })
        })
        .collect();
    resegment(loaded, source_lang, target_lang)
}

/// Java `ParseEntry.stripSomeChars` (spaces + CR/LF normalize).
fn strip_some_chars(src: &str, remove_spaces: bool) -> String {
    let mut r = src.to_string();
    if remove_spaces {
        r = r
            .trim_matches(|c: char| c.is_whitespace() || c == '\u{00A0}')
            .to_string();
    }
    r = r.replace("\r\n", "\n").replace('\r', "\n");
    r
}

fn resegment(entries: Vec<TmxEntry>, source_lang: &str, target_lang: &str) -> Vec<TmxEntry> {
    let mut out = Vec::new();
    for e in entries {
        let srcs = crate::segment::segment_sentences_lang(&e.source, true, source_lang, None);
        let tgts = crate::segment::segment_sentences_lang(&e.translation, true, target_lang, None);
        // Java `Segmenter.segmentEntries`: only keep the split when both sides
        // produce the same sentence count (empty trimmed chunks still count).
        if srcs.len() > 1 && srcs.len() == tgts.len() {
            for i in 0..srcs.len() {
                if srcs[i].trim().is_empty() {
                    continue;
                }
                let mut next = e.clone();
                next.source = srcs[i].clone();
                next.translation = tgts[i].clone();
                out.push(next);
            }
        } else {
            out.push(e);
        }
    }
    out
}
