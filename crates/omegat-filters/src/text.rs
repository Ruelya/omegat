use crate::{
    apply_skeleton, ensure_parent, placeholder, read_to_string, ExtractedSegment, Filter,
    FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct TextFilter;

impl Filter for TextFilter {
    fn id(&self) -> &'static str {
        "text"
    }
    fn name(&self) -> &'static str {
        "Text"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.txt"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let raw = read_to_string(path)?;
        parse_text(&raw)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let parsed = parse_text(&raw)?;
        let out = if let Some(sk) = parsed.skeleton {
            apply_skeleton(&sk, translations)
        } else {
            raw
        };
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_text(raw: &str) -> Result<ParsedFile> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let parts: Vec<&str> = normalized.split("\n\n").collect();
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            skeleton.push_str("\n\n");
        }
        let text = part.trim_end_matches('\n');
        if text.trim().is_empty() {
            skeleton.push_str(part);
            continue;
        }
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: text.to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: None,
            protected_parts: vec![],
        });
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}
