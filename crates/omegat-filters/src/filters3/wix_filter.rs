//! Java `WiXFilter`.

use crate::xml_engine::FilterHooks;
use crate::xml_filter::{parse_xml, write_xml};
use crate::{ExtractedSegment, Filter, FilterContext, ParsedFile, Result};
use std::collections::HashMap;
use std::path::Path;

use super::wix_dialect::WiXDialect;

pub struct WiXFilter;

struct WixHooks {
    segments: Vec<ExtractedSegment>,
    translations: HashMap<String, String>,
    collect: bool,
    id: Option<String>,
}

impl FilterHooks for WixHooks {
    fn tag_start(&mut self, _path: &str, attrs: &[(String, String)]) {
        self.id = attrs
            .iter()
            .find(|(n, _)| n == "Id")
            .map(|(_, v)| v.clone());
    }
    fn tag_end(&mut self, _path: &str) {}
    fn comment(&mut self, _comment: &str) {}
    fn text(&mut self, _text: &str) {}
    fn is_in_ignored(&self) -> bool {
        false
    }
    fn translate(&mut self, entry: &str, protected: &[crate::ProtectedPart]) -> String {
        if entry.is_empty() {
            return String::new();
        }
        if self.collect {
            let id = self
                .id
                .clone()
                .unwrap_or_else(|| self.segments.len().to_string());
            self.segments.push(ExtractedSegment {
                id: id.clone(),
                source: entry.to_string(),
                existing_translation: None,
                note: None,
                comment: None,
                path: None,
                protected_parts: protected.to_vec(),
            });
            entry.to_string()
        } else {
            self.id
                .as_ref()
                .and_then(|id| self.translations.get(id).cloned())
                .or_else(|| self.translations.get(entry).cloned())
                .unwrap_or_else(|| entry.to_string())
        }
    }
}

impl Filter for WiXFilter {
    fn id(&self) -> &'static str {
        "wix"
    }
    fn name(&self) -> &'static str {
        "WiX Localization"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.wxl"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let dialect = WiXDialect::new();
        let mut hooks = WixHooks {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            id: None,
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
        let dialect = WiXDialect::new();
        let mut hooks = WixHooks {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            id: None,
        };
        write_xml(source_path, dest_path, &dialect, &mut hooks)
    }
}
