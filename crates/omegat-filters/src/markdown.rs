use crate::{
    apply_skeleton, ensure_parent, placeholder, read_to_string, ExtractedSegment, Filter,
    FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct MarkdownFilter;

impl Filter for MarkdownFilter {
    fn id(&self) -> &'static str {
        "markdown"
    }
    fn name(&self) -> &'static str {
        "Markdown"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.md", "*.markdown"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_md(&read_to_string(path)?)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let parsed = parse_md(&read_to_string(source_path)?)?;
        let out = parsed
            .skeleton
            .map(|sk| apply_skeleton(&sk, translations))
            .unwrap_or_default();
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_md(raw: &str) -> Result<ParsedFile> {
    let normalized = raw.replace("\r\n", "\n");
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut in_code = false;
    let mut para = String::new();
    let flush = |para: &mut String, segments: &mut Vec<ExtractedSegment>, skeleton: &mut String| {
        let text = para.trim_end().to_string();
        para.clear();
        if text.is_empty() {
            return;
        }
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: text,
            existing_translation: None,
            note: None,
            comment: None,
            path: None,
            protected_parts: vec![],
        });
    };
    for line in normalized.lines() {
        if line.starts_with("```") {
            flush(&mut para, &mut segments, &mut skeleton);
            in_code = !in_code;
            skeleton.push_str(line);
            skeleton.push('\n');
            continue;
        }
        if in_code {
            skeleton.push_str(line);
            skeleton.push('\n');
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut para, &mut segments, &mut skeleton);
            skeleton.push('\n');
            continue;
        }
        if !para.is_empty() {
            para.push('\n');
        }
        para.push_str(line);
    }
    flush(&mut para, &mut segments, &mut skeleton);
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}
