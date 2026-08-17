use crate::{
    apply_skeleton_with_originals, ensure_parent, merge_translations, placeholder, read_to_string,
    ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
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
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        parse_text(&read_to_string(path)?, ctx.option("segmentOn").unwrap_or("EMPTYLINES"))
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let parsed = parse_text(&raw, ctx.option("segmentOn").unwrap_or("EMPTYLINES"))?;
        let merged = merge_translations(&parsed.segments, translations);
        let originals: Vec<String> = parsed.segments.iter().map(|s| s.source.clone()).collect();
        let out = parsed
            .skeleton
            .map(|sk| apply_skeleton_with_originals(&sk, &merged, &originals))
            .unwrap_or(raw);
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_text(raw: &str, segment_on: &str) -> Result<ParsedFile> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mode = segment_on.to_ascii_uppercase();
    let parts: Vec<String> = match mode.as_str() {
        "NEVER" => {
            let t = normalized.trim_end_matches('\n');
            if t.is_empty() {
                vec![]
            } else {
                vec![t.to_string()]
            }
        }
        "BREAKS" => normalized
            .split('\n')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => normalized
            .split("\n\n")
            .map(|s| s.trim_end_matches('\n').to_string())
            .filter(|s| !s.trim().is_empty())
            .collect(),
    };

    let mut segments = Vec::new();
    let mut skeleton = String::new();
    if mode == "NEVER" {
        if let Some(text) = parts.first() {
            skeleton.push_str(&placeholder(0));
            if raw.ends_with('\n') {
                skeleton.push('\n');
            }
            segments.push(text_seg(0, text));
        }
        return Ok(ParsedFile {
            segments,
            skeleton: Some(skeleton),
        });
    }

    if mode == "BREAKS" {
        let lines: Vec<&str> = normalized.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                skeleton.push('\n');
            }
            if line.is_empty() {
                continue;
            }
            skeleton.push_str(&placeholder(segments.len()));
            segments.push(text_seg(segments.len(), line));
        }
        return Ok(ParsedFile {
            segments,
            skeleton: Some(skeleton),
        });
    }

    let chunks: Vec<&str> = normalized.split("\n\n").collect();
    for (i, part) in chunks.iter().enumerate() {
        if i > 0 {
            skeleton.push_str("\n\n");
        }
        let text = part.trim_end_matches('\n');
        if text.trim().is_empty() {
            skeleton.push_str(part);
            continue;
        }
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(text_seg(segments.len(), text));
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn text_seg(i: usize, source: &str) -> ExtractedSegment {
    ExtractedSegment {
        id: i.to_string(),
        source: source.to_string(),
        existing_translation: None,
        note: None,
        comment: None,
        path: None,
        protected_parts: vec![],
    }
}
