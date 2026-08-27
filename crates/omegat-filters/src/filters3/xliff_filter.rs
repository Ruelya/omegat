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
    alt_ids: HashSet<String>,
}

impl FilterHooks for XliffHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]) {
        if path.ends_with("trans-unit") {
            self.resname = attrs
                .iter()
                .find(|(n, _)| n == "resname")
                .map(|(_, v)| v.clone());
            self.id = attrs.iter().find(|(n, _)| n == "id").map(|(_, v)| v.clone());
        }
        if path == "/xliff/file/header" {
            self.ignored = true;
        }
    }

    fn tag_end(&mut self, path: &str) {
        if path.ends_with("trans-unit") {
            if self.collect {
                for src in self.entry_text.drain(..) {
                    let id = self
                        .id
                        .clone()
                        .unwrap_or_else(|| self.segments.len().to_string());
                    self.segments.push(ExtractedSegment {
                        id,
                        source: src,
                        existing_translation: None,
                        note: self.resname.clone(),
                        comment: None,
                        path: None,
                        protected_parts: vec![],
                    });
                }
            }
            self.id = None;
            self.resname = None;
            self.entry_text.clear();
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

    fn translate(&mut self, entry: &str, _protected: &[ProtectedPart]) -> String {
        if entry.is_empty() {
            return String::new();
        }
        if self.collect {
            self.entry_text.push(entry.to_string());
            entry.to_string()
        } else {
            self.translations
                .get(entry)
                .cloned()
                .or_else(|| self.id.as_ref().and_then(|id| self.translations.get(id).cloned()))
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
            alt_ids: HashSet::new(),
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
            alt_ids: HashSet::new(),
        };
        write_xml_cfg(source_path, dest_path, &dialect, &mut hooks, engine_config(ctx))
    }
}
