//! Java `ResXFilter`.

use crate::xml_engine::FilterHooks;
use crate::xml_filter::{parse_xml, write_xml};
use crate::{ExtractedSegment, Filter, FilterContext, ParsedFile, ProtectedPart, Result};
use std::collections::HashMap;
use std::path::Path;

use super::resx_dialect::ResXDialect;

pub struct ResXFilter;

struct ResxHooks {
    segments: Vec<ExtractedSegment>,
    translations: HashMap<String, String>,
    collect: bool,
    id: Option<String>,
    entry_text: Option<String>,
    comment: Option<String>,
    text: Option<String>,
}

impl FilterHooks for ResxHooks {
    fn tag_start(&mut self, path: &str, attrs: &[(String, String)]) {
        if path == "/root/data" {
            self.id = attrs
                .iter()
                .find(|(n, _)| n == "name")
                .map(|(_, v)| v.clone())
                .or_else(|| Some(String::new()));
            self.comment = None;
        }
    }

    fn tag_end(&mut self, path: &str) {
        if path == "/root/data/comment" {
            self.comment = self.text.clone();
        } else if path == "/root/data" {
            if self.collect {
                if let Some(entry) = self.entry_text.take() {
                    self.segments.push(ExtractedSegment {
                        id: self.id.clone().unwrap_or_default(),
                        source: entry,
                        existing_translation: None,
                        note: self.comment.clone(),
                        comment: self.comment.clone(),
                        path: None,
                        protected_parts: vec![],
                    });
                }
            }
            self.id = None;
            self.entry_text = None;
            self.comment = None;
        }
    }

    fn comment(&mut self, _comment: &str) {}

    fn text(&mut self, text: &str) {
        self.text = Some(text.to_string());
    }

    fn is_in_ignored(&self) -> bool {
        false
    }

    fn translate(&mut self, entry: &str, _protected: &[ProtectedPart]) -> String {
        if self.collect {
            self.entry_text = Some(entry.to_string());
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

impl Filter for ResXFilter {
    fn id(&self) -> &'static str {
        "resx"
    }
    fn name(&self) -> &'static str {
        "ResX"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.resx"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = ResXDialect::new();
        let mut hooks = ResxHooks {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            id: None,
            entry_text: None,
            comment: None,
            text: None,
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
        let dialect = ResXDialect::new();
        let mut hooks = ResxHooks {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            id: None,
            entry_text: None,
            comment: None,
            text: None,
        };
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
