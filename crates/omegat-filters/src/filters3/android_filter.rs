//! Java `AndroidFilter`.

use crate::xml_engine::FilterHooks;
use crate::xml_filter::{parse_xml, write_xml};
use crate::{ExtractedSegment, Filter, FilterContext, ParsedFile, ProtectedPart, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::android_dialect::AndroidDialect;

const NAMED_TAGS: &[&str] = &[
    "/resources/string",
    "/resources/color",
    "/resources/array",
    "/resources/string-array",
    "/resources/integer-array",
];

pub struct AndroidFilter;

struct AndroidHooks {
    segments: Vec<ExtractedSegment>,
    translations: HashMap<String, String>,
    collect: bool,
    id: Option<String>,
    id_plurals: String,
    comment: Option<String>,
    id_comment: Option<String>,
}

impl AndroidHooks {
    fn named() -> HashSet<&'static str> {
        NAMED_TAGS.iter().copied().collect()
    }
}

impl FilterHooks for AndroidHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]) {
        let named = Self::named();
        if named.contains(path) {
            self.id = attrs
                .iter()
                .find(|(n, _)| n == "name")
                .map(|(_, v)| v.clone());
            self.id_comment = self.comment.clone();
        } else if path == "/resources/plurals" {
            self.id_plurals = attrs
                .iter()
                .find(|(n, _)| n == "name")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
        } else if path == "/resources/plurals/item" {
            let qty = attrs
                .iter()
                .find(|(n, _)| n == "quantity")
                .map(|(_, v)| v.as_str())
                .unwrap_or("");
            self.id = Some(format!("{}/{}", self.id_plurals, qty));
            self.id_comment = self.comment.clone();
        }
    }

    fn tag_end(&mut self, path: &str) {
        self.comment = None;
        if path == "/resources/string" || path == "/resources/plurals/item" {
            self.id_comment = None;
        }
    }

    fn comment(&mut self, comment: &str) {
        self.comment = Some(match &self.comment {
            None => comment.to_string(),
            Some(prev) => format!("{prev}\n{comment}"),
        });
    }

    fn text(&mut self, _text: &str) {}

    fn is_in_ignored(&self) -> bool {
        false
    }

    fn translate(&mut self, entry: &str, protected: &[ProtectedPart]) -> String {
        if let Some(c) = &self.id_comment {
            let low = c.to_ascii_lowercase();
            if low.contains("do not translate") || low.contains("don't translate") {
                return entry.to_string();
            }
        }
        let e = entry.replace("\\'", "'");
        let mut r = e.clone();
        if e.is_empty() {
            return r;
        }
        if self.collect {
            if !e.is_empty() {
                self.segments.push(ExtractedSegment {
                    id: self.id.clone().unwrap_or_else(|| self.segments.len().to_string()),
                    source: e,
                    existing_translation: None,
                    note: self.id_comment.clone(),
                    comment: self.id_comment.clone(),
                    path: None,
                    protected_parts: protected.to_vec(),
                });
            }
        } else {
            let translation = self
                .id
                .as_ref()
                .and_then(|id| self.translations.get(id))
                .or_else(|| self.translations.get(&e));
            if let Some(t) = translation {
                if !t.is_empty() {
                    r = t.clone();
                }
            }
        }
        r.replace('\'', "\\'")
    }
}

impl Filter for AndroidFilter {
    fn id(&self) -> &'static str {
        "android"
    }
    fn name(&self) -> &'static str {
        "Android Resources"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = AndroidDialect::new();
        let mut hooks = AndroidHooks {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            id: None,
            id_plurals: String::new(),
            comment: None,
            id_comment: None,
        };
        parse_xml(path, &dialect, &mut hooks)?;
        Ok(ParsedFile {
            segments: hooks.segments,
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = AndroidDialect::new();
        let mut hooks = AndroidHooks {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            id: None,
            id_plurals: String::new(),
            comment: None,
            id_comment: None,
        };
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
