use crate::{
    apply_skeleton_with_originals, ensure_parent, placeholder, read_to_string, ExtractedSegment, Filter,
    FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct SrtFilter;
pub struct SbvFilter;
pub struct WebVttFilter;

impl Filter for SrtFilter {
    fn id(&self) -> &'static str {
        "srt"
    }
    fn name(&self) -> &'static str {
        "SubRip Subtitles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.srt"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_blocks(&read_to_string(path)?, |line| {
            line.parse::<u32>().is_ok() || line.contains("-->")
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        write_blocks(source_path, dest_path, translations, |line| {
            line.parse::<u32>().is_ok() || line.contains("-->")
        })
    }
}

impl Filter for SbvFilter {
    fn id(&self) -> &'static str {
        "sbv"
    }
    fn name(&self) -> &'static str {
        "YouTube Subtitles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.sbv"]
    }
    fn phase(&self) -> u8 {
        3
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_blocks(&read_to_string(path)?, |line| line.contains(','))
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        write_blocks(source_path, dest_path, translations, |line| line.contains(','))
    }
}

impl Filter for WebVttFilter {
    fn id(&self) -> &'static str {
        "webvtt"
    }
    fn name(&self) -> &'static str {
        "WebVTT Subtitles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.vtt"]
    }
    fn phase(&self) -> u8 {
        3
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_blocks(&read_to_string(path)?, |line| {
            line.starts_with("WEBVTT") || line.contains("-->") || line.starts_with("NOTE")
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        write_blocks(source_path, dest_path, translations, |line| {
            line.starts_with("WEBVTT") || line.contains("-->") || line.starts_with("NOTE")
        })
    }
}

fn parse_blocks(raw: &str, is_meta: fn(&str) -> bool) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, segments: &mut Vec<ExtractedSegment>, skeleton: &mut String| {
        let text = buf.trim_end().to_string();
        buf.clear();
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
    for line in raw.lines() {
        if is_meta(line) || line.trim().is_empty() {
            flush(&mut buf, &mut segments, &mut skeleton);
            skeleton.push_str(line);
            skeleton.push('\n');
        } else {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(line);
        }
    }
    flush(&mut buf, &mut segments, &mut skeleton);
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

fn write_blocks(
    source_path: &Path,
    dest_path: &Path,
    translations: &HashMap<String, String>,
    is_meta: fn(&str) -> bool,
) -> Result<()> {
    let parsed = parse_blocks(&read_to_string(source_path)?, is_meta)?;
    let originals: Vec<String> = parsed.segments.iter().map(|s| s.source.clone()).collect();
    let out = parsed
        .skeleton
        .map(|sk| apply_skeleton_with_originals(&sk, translations, &originals))
        .unwrap_or_default();
    ensure_parent(dest_path)?;
    std::fs::write(dest_path, out)?;
    Ok(())
}
