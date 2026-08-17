//! Plain-text filter. Line/paragraph rules follow Java `TextFilter`.

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

/// Java `LinebreakPreservingReader` + `processSegEmptyLines` / `processSegLineBreaks` / `processNonSeg`.
fn parse_text(raw: &str, segment_on: &str) -> Result<ParsedFile> {
    let mode = segment_on.to_ascii_uppercase();
    match mode.as_str() {
        "NEVER" => parse_never(raw),
        "BREAKS" => parse_breaks(raw),
        _ => parse_empty_lines(raw),
    }
}

fn parse_never(raw: &str) -> Result<ParsedFile> {
    if raw.is_empty() {
        return Ok(ParsedFile {
            segments: vec![],
            skeleton: Some(String::new()),
        });
    }
    Ok(ParsedFile {
        segments: vec![text_seg(0, raw)],
        skeleton: Some(placeholder(0)),
    })
}

fn parse_breaks(raw: &str) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut nontrans = String::new();
    for (line, br) in lines_with_breaks(raw) {
        if line.trim().is_empty() {
            nontrans.push_str(line);
            nontrans.push_str(br);
            continue;
        }
        skeleton.push_str(&nontrans);
        nontrans.clear();
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(text_seg(segments.len(), line));
        nontrans.push_str(br);
    }
    skeleton.push_str(&nontrans);
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn parse_empty_lines(raw: &str) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut nontrans = String::new();
    let mut trans = String::new();
    for (line, br) in lines_with_breaks(raw) {
        if line.is_empty() {
            skeleton.push_str(&nontrans);
            nontrans.clear();
            if !trans.is_empty() {
                skeleton.push_str(&placeholder(segments.len()));
                segments.push(text_seg(segments.len(), &trans));
                trans.clear();
            }
            nontrans.push_str(br);
        } else if line.trim().is_empty() && trans.is_empty() {
            nontrans.push_str(line);
            nontrans.push_str(br);
        } else {
            trans.push_str(line);
            trans.push_str(br);
        }
    }
    skeleton.push_str(&nontrans);
    if !trans.is_empty() {
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(text_seg(segments.len(), &trans));
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn lines_with_breaks(raw: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    let mut start = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            out.push((&raw[start..i], &raw[i..i + 2]));
            i += 2;
            start = i;
        } else if bytes[i] == b'\n' || bytes[i] == b'\r' {
            out.push((&raw[start..i], &raw[i..i + 1]));
            i += 1;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < raw.len() {
        out.push((&raw[start..], ""));
    }
    out
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
