//! Java `XLIFFFilter` (filters3).

use crate::xml_engine::FilterHooks;
use crate::xml_filter::{engine_config, parse_xml_cfg, write_xml_cfg};
use crate::{ExtractedSegment, Filter, FilterContext, ParsedFile, ProtectedPart, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::xliff_dialect::XliffDialect;

pub struct XliffFilter;

struct XliffHooks {
    segments: Vec<ExtractedSegment>,
    translations: HashMap<String, String>,
    collect: bool,
    id: Option<String>,
    resname: Option<String>,
    ignored: bool,
    entry_text: Vec<String>,
    entry_protected_parts: Vec<Vec<ProtectedPart>>,
    alt_ids: HashSet<String>,
    next_unit_id: usize,
    entry_ordinal: usize,
}

impl XliffHooks {
    fn unique_unit_id(&mut self, candidate: String) -> String {
        let mut suffix = 0usize;
        loop {
            let id = if suffix == 0 {
                candidate.clone()
            } else {
                format!("{candidate}_{suffix}")
            };
            if self.alt_ids.insert(id.clone()) {
                return id;
            }
            suffix += 1;
        }
    }
}

impl FilterHooks for XliffHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]) {
        if path.ends_with("trans-unit") {
            self.resname = attrs
                .iter()
                .find(|(n, _)| n == "resname")
                .map(|(_, v)| v.clone());
            let candidate = attrs
                .iter()
                .find(|(n, _)| n == "id")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| self.next_unit_id.to_string());
            self.id = Some(self.unique_unit_id(candidate));
            self.next_unit_id += 1;
            self.entry_ordinal = 0;
        }
        if path == "/xliff/file/header" {
            self.ignored = true;
        }
    }

    fn tag_end(&mut self, path: &str) {
        if path.ends_with("trans-unit") {
            if self.collect {
                let entry_count = self.entry_text.len();
                let base_id = self
                    .id
                    .clone()
                    .unwrap_or_else(|| self.segments.len().to_string());
                for (ordinal, src) in self.entry_text.drain(..).enumerate() {
                    let id = if entry_count > 1 {
                        format!("{base_id}#{ordinal}")
                    } else {
                        base_id.clone()
                    };
                    self.segments.push(ExtractedSegment {
                        id,
                        source: src,
                        existing_translation: None,
                        note: self.resname.clone(),
                        comment: None,
                        path: None,
                        protected_parts: self
                            .entry_protected_parts
                            .get(ordinal)
                            .cloned()
                            .unwrap_or_default(),
                    });
                }
            }
            self.id = None;
            self.resname = None;
            self.entry_text.clear();
            self.entry_protected_parts.clear();
            self.entry_ordinal = 0;
        }
        if path == "/xliff/file/header" {
            self.ignored = false;
        }
        if path.ends_with("/file") {
            self.alt_ids.clear();
        }
    }

    fn comment(&mut self, _comment: &str) {}
    fn text(&mut self, _text: &str) {}
    fn is_in_ignored(&self) -> bool {
        self.ignored
    }

    fn translate(&mut self, entry: &str, protected: &[ProtectedPart]) -> String {
        if entry.is_empty() {
            return String::new();
        }
        if self.collect {
            self.entry_text.push(entry.to_string());
            self.entry_protected_parts.push(protected.to_vec());
            entry.to_string()
        } else {
            let ordinal = self.entry_ordinal;
            self.entry_ordinal += 1;
            let base_id = self.id.as_deref().unwrap_or("0");
            self.translations
                .get(&format!("{base_id}#{ordinal}"))
                .cloned()
                .or_else(|| {
                    (ordinal == 0)
                        .then(|| self.translations.get(base_id).cloned())
                        .flatten()
                })
                .or_else(|| self.translations.get(entry).cloned())
                .unwrap_or_else(|| entry.to_string())
        }
    }
}

impl Filter for XliffFilter {
    fn id(&self) -> &'static str {
        "xliff"
    }
    fn name(&self) -> &'static str {
        "XLIFF"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.xlf", "*.xliff"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = XliffDialect::new(&ctx.options);
        let mut hooks = XliffHooks {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            id: None,
            resname: None,
            ignored: false,
            entry_text: Vec::new(),
            entry_protected_parts: Vec::new(),
            alt_ids: HashSet::new(),
            next_unit_id: 0,
            entry_ordinal: 0,
        };
        parse_xml_cfg(path, &dialect, &mut hooks, engine_config(ctx))?;
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
        ctx: &FilterContext,
    ) -> Result<()> {
        let dialect = XliffDialect::new(&ctx.options);
        let mut hooks = XliffHooks {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            id: None,
            resname: None,
            ignored: false,
            entry_text: Vec::new(),
            entry_protected_parts: Vec::new(),
            alt_ids: HashSet::new(),
            next_unit_id: 0,
            entry_ordinal: 0,
        };
        write_xml_cfg(
            source_path,
            dest_path,
            &dialect,
            &mut hooks,
            engine_config(ctx),
        )
    }
}
