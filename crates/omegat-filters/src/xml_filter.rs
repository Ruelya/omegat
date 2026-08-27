//! Java `XMLFilter` wrapper: parse/write through the event engine.

use crate::xml_dialect::{file_looks_like, XmlDialect};
use crate::xml_engine::{EngineConfig, FilterHooks, ProcessResult};
use crate::{
    ensure_parent, read_to_string, ExtractedSegment, FilterContext, ParsedFile, ProtectedPart,
    Result,
};
use std::collections::HashMap;
use std::path::Path;

fn run_xml(
    raw: &str,
    path: Option<&Path>,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
    inline_system: bool,
) -> Result<String> {
    let base = path.and_then(|p| p.parent());
    let mut owned = raw.to_string();
    if let Some(p) = path {
        if let Ok(bytes) = std::fs::read(p) {
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) && !owned.starts_with('\u{feff}') {
                owned.insert(0, '\u{feff}');
            }
        }
    }
    let ProcessResult { output } =
        crate::xml_engine::process_xml_ex(&owned, dialect, hooks, cfg, base, inline_system)
            .map_err(|e| crate::FilterError::Parse {
                format: "xml".into(),
                message: e,
            })?;
    Ok(output)
}

pub struct DefaultHooks {
    pub segments: Vec<ExtractedSegment>,
    pub translations: HashMap<String, String>,
    pub collect: bool,
    pub current_id: Option<String>,
    pub current_comment: Option<String>,
    id_prefix: String,
    next_id: usize,
}

impl DefaultHooks {
    pub fn parse() -> Self {
        Self {
            segments: Vec::new(),
            translations: HashMap::new(),
            collect: true,
            current_id: None,
            current_comment: None,
            id_prefix: String::new(),
            next_id: 0,
        }
    }

    pub fn parse_with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            id_prefix: prefix.into(),
            ..Self::parse()
        }
    }

    pub fn write(translations: &HashMap<String, String>) -> Self {
        Self {
            segments: Vec::new(),
            translations: translations.clone(),
            collect: false,
            current_id: None,
            current_comment: None,
            id_prefix: String::new(),
            next_id: 0,
        }
    }

    pub fn write_with_prefix(
        translations: &HashMap<String, String>,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            id_prefix: prefix.into(),
            ..Self::write(translations)
        }
    }

    pub(crate) fn enter_part(&mut self, prefix: impl Into<String>) {
        self.current_id = None;
        self.current_comment = None;
        self.id_prefix = prefix.into();
        self.next_id = 0;
    }

    fn next_segment_id(&mut self) -> String {
        let id = self
            .current_id
            .clone()
            .unwrap_or_else(|| format!("{}{}", self.id_prefix, self.next_id));
        self.next_id += 1;
        id
    }

    fn lookup(&self, source: &str, id: &str) -> String {
        if let Some(translation) = self.translations.get(id) {
            if !translation.is_empty() {
                return translation.clone();
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
        let id = self.next_segment_id();
        if self.collect {
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
            self.lookup(entry, &id)
        }
    }
}

pub fn engine_config(ctx: &FilterContext) -> EngineConfig {
    EngineConfig {
        remove_tags: ctx.remove_tags,
        remove_spaces_nonseg: ctx.remove_spaces_nonseg,
        preserve_spaces: false,
    }
}

pub fn parse_xml(
    path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
) -> Result<String> {
    parse_xml_cfg(path, dialect, hooks, EngineConfig::default())
}

pub fn parse_xml_cfg(
    path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
) -> Result<String> {
    let raw = read_to_string(path)?;
    run_xml(&raw, Some(path), dialect, hooks, cfg, true)
}

pub fn write_xml(
    source_path: &Path,
    dest_path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
) -> Result<()> {
    write_xml_cfg(
        source_path,
        dest_path,
        dialect,
        hooks,
        EngineConfig::default(),
    )
}

pub fn write_xml_cfg(
    source_path: &Path,
    dest_path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut dyn FilterHooks,
    cfg: EngineConfig,
) -> Result<()> {
    let raw = read_to_string(source_path)?;
    let output = run_xml(&raw, Some(source_path), dialect, hooks, cfg, false)?;
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, output)?;
    Ok(())
}

pub fn parse_to_file(
    path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
) -> Result<ParsedFile> {
    parse_to_file_cfg(path, dialect, hooks, EngineConfig::default())
}

pub fn parse_to_file_cfg(
    path: &Path,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
    cfg: EngineConfig,
) -> Result<ParsedFile> {
    let raw = read_to_string(path)?;
    parse_raw_cfg_at(&raw, Some(path), dialect, hooks, cfg, true)
}

pub fn parse_raw(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
) -> Result<ParsedFile> {
    parse_raw_cfg(raw, dialect, hooks, EngineConfig::default())
}

pub fn parse_raw_cfg(
    raw: &str,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
    cfg: EngineConfig,
) -> Result<ParsedFile> {
    parse_raw_cfg_at(raw, None, dialect, hooks, cfg, false)
}

fn parse_raw_cfg_at(
    raw: &str,
    path: Option<&Path>,
    dialect: &dyn XmlDialect,
    hooks: &mut DefaultHooks,
    cfg: EngineConfig,
    inline_system: bool,
) -> Result<ParsedFile> {
    let output = run_xml(raw, path, dialect, hooks, cfg, inline_system)?;
    Ok(ParsedFile {
        segments: std::mem::take(&mut hooks.segments),
        skeleton: Some(output),
    })
}

pub fn dialect_supports(raw: &str, dialect: &dyn XmlDialect) -> bool {
    file_looks_like(raw, dialect)
}
