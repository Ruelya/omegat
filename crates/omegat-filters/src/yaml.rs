use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct YamlFilter;

impl Filter for YamlFilter {
    fn id(&self) -> &'static str {
        "yaml"
    }
    fn name(&self) -> &'static str {
        "YAML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.yaml", "*.yml"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_yaml(&read_to_string(path)?)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let parsed = parse_yaml(&raw)?;
        let mut out = raw;
        for seg in parsed.segments {
            if let Some(t) = translations.get(&seg.id) {
                if let Some(pos) = out.find(&seg.source) {
                    out.replace_range(pos..pos + seg.source.len(), t);
                }
            }
        }
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_yaml(raw: &str) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((_, rest)) = trimmed.split_once(':') {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if v.is_empty() || v == "|" || v == ">" || v.starts_with('[') || v.starts_with('{') {
                continue;
            }
            let key = trimmed.split(':').next().unwrap_or("").trim();
            segments.push(ExtractedSegment {
                id: if key.is_empty() {
                    segments.len().to_string()
                } else {
                    key.to_string()
                },
                source: v.to_string(),
                existing_translation: None,
                note: None,
                comment: None,
                path: Some(key.to_string()),
                protected_parts: vec![],
            });
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}
