//! Java `XMLFilter` wrapper: parse/write through the event engine.

use crate::xml_dialect::{file_looks_like, XmlDialect};
use crate::xml_engine::{EngineConfig, FilterHooks, ProcessResult};
use crate::{
    ensure_parent, read_to_string, ExtractedSegment, FilterContext, ParsedFile, ProtectedPart,
    Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct DefaultHooks {
    pub segments: Vec<ExtractedSegment>,
    pub translations: HashMap<String, String>,
    pub collect: bool,
    pub current_id: Option<String>,
    pub current_comment: Option<String>,
}

impl DefaultHooks {
    pub fn parse() -> Self {
        Self {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            current_id: None,
            current_comment: None,
        }
    }

    pub fn write(translations: &HashMap<String, String>) -> Self {
        Self {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            current_id: None,
            current_comment: None,
        }
    }

    fn lookup(&self, source: &str) -> String {
        if let Some(id) = &self.current_id {
            if let Some(t) = self.translations.get(id) {
                if !t.is_empty() {
                    return t.clone();
                }
            }
        }
        self.translations
            .get(source)
            .cloned()
            .unwrap_or_else(|| source.to_string())
    }
}

impl FilterHooks for DefaultHooks {
    fn tag_start(&mut self, _path: &str, _attrs: &[(String, String)]) {}
    fn tag_end(&mut self, _path: &str) {}
    fn comment(&mut self, _comment: &str) {}
    fn text(&mut self, _text: &str) {}
    fn is_in_ignored(&self) -> bool {
        false
    }
    fn translate(&mut self, entry: &str, protected: &[ProtectedPart]) -> String {
        if entry.is_empty() {
            return String::new();
        }
        if self.collect {
            let id = self
                .current_id
                .clone()
                .unwrap_or_else(|| self.segments.len().to_string());
            self.segments.push(ExtractedSegment {
                id,
                source: entry.to_string(),
                existing_translation: None,
                note: self.current_comment.clone(),
                comment: self.current_comment.clone(),
                path: None,
                protected_parts: protected.to_vec(),
            });
            entry.to_string()
        } else {
            self.lookup(entry)
        }
    }
}

pub fn engine_config(_ctx: &FilterContext) -> EngineConfig {
    EngineConfig::default()
}

pub fn parse_xml(path: &Path, dialect: &dyn XmlDialect, hooks: &mut dyn FilterHooks) -> Result<String> {
    let raw = read_to_string(path)?;
    let ProcessResult { output } =
        crate::xml_engine::process_xml(&raw, dialect, hooks, EngineConfig::default()).map_err(
            |e| crate::FilterError::Parse {
                format: "xml".into(),
                message: e,
            },
        )?;
    Ok(output)
}

pub fn write_xml(
    source_path: &Path,
    dest_path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
) -> Result<()> {
    let raw = read_to_string(source_path)?;
    let ProcessResult { output } =
        crate::xml_engine::process_xml(&raw, dialect, hooks, EngineConfig::default()).map_err(
            |e| crate::FilterError::Parse {
                format: "xml".into(),
                message: e,
            },
        )?;
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, output)?;
    Ok(())
}

pub fn parse_to_file(
    path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
) -> Result<ParsedFile> {
    let raw = read_to_string(path)?;
    parse_raw(&raw, dialect, hooks)
}

pub fn parse_raw(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
) -> Result<ParsedFile> {
    let ProcessResult { output } =
        crate::xml_engine::process_xml(raw, dialect, hooks, EngineConfig::default()).map_err(|e| {
            crate::FilterError::Parse {
                format: "xml".into(),
                message: e,
            }
        })?;
    Ok(ParsedFile {
        segments: std::mem::take(&mut hooks.segments),
        skeleton: Some(output),
    })
}

pub fn dialect_supports(raw: &str, dialect: &dyn XmlDialect) -> bool {
    file_looks_like(raw, dialect)
}
