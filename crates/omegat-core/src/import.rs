//! Java `RealProject.importTranslationsFromSources`.

use crate::tmx::{ProjectTmx, TmxEntry};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SourceImport {
    pub id: String,
    pub source: String,
    pub source_translation: Option<String>,
    pub fuzzy: bool,
}

/// Import existing translations from source-file STEs into project TMX.
pub fn import_translations_from_sources(
    tmx: &mut ProjectTmx,
    entries: &[SourceImport],
    support_default: bool,
    allow_equal_to_source: bool,
) {
    let mut allow_to_import: HashMap<String, String> = HashMap::new();
    for ste in entries {
        let Some(tr) = ste.source_translation.as_deref() else {
            continue;
        };
        if ste.fuzzy {
            continue;
        }
        if ste.source == tr && !allow_equal_to_source {
            continue;
        }
        if support_default {
            if tmx.get_multiple_translation(&ste.id, &ste.source).is_some() {
                continue;
            }
            if tmx.get_default_translation(&ste.source).is_none() {
                tmx.set_default_translation(&ste.source, tr);
                allow_to_import.insert(ste.source.clone(), tr.to_string());
            } else if let Some(just) = allow_to_import.get(&ste.source) {
                if tr != just {
                    tmx.set_multiple_translation(&ste.id, &ste.source, tr);
                }
            }
        } else if tmx.get_multiple_translation(&ste.id, &ste.source).is_none() {
            tmx.set_multiple_translation(&ste.id, &ste.source, tr);
        }
    }
}

pub fn default_of<'a>(tmx: &'a ProjectTmx, source: &str) -> Option<&'a TmxEntry> {
    tmx.get_default_translation(source)
}

pub fn alternative_of<'a>(tmx: &'a ProjectTmx, id: &str, source: &str) -> Option<&'a TmxEntry> {
    tmx.get_multiple_translation(id, source)
}
