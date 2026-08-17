//! Shared subtitle helpers. Timed formats follow Java `SrtFilter.processFile`.

use crate::{
    apply_skeleton_with_originals, ensure_parent, placeholder, read_to_string, ExtractedSegment,
    ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
pub fn parse_blocks(raw: &str, is_meta: fn(&str) -> bool) -> Result<ParsedFile> {
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

pub struct TimedOutcome {
    pub parsed: ParsedFile,
    pub written: String,
}

/// Java `SrtFilter` / `SbvFilter` / `WebVttFilter` state machine. Time line is the id.
pub fn process_timed(
    raw: &str,
    time_re: &regex::Regex,
    translations: Option<&HashMap<String, String>>,
) -> TimedOutcome {
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut wait_text = false;
    let mut key = String::new();
    let mut text = String::new();
    const EOL: &str = "\r\n";

    let flush = |key: &str,
                 text: &str,
                 segments: &mut Vec<ExtractedSegment>,
                 written: &mut String,
                 translations: Option<&HashMap<String, String>>| {
        if text.is_empty() {
            return;
        }
        segments.push(ExtractedSegment {
            id: key.to_string(),
            source: text.to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: None,
            protected_parts: vec![],
        });
        let tr = translations
            .and_then(|m| m.get(key).or_else(|| m.get(text)).cloned())
            .unwrap_or_else(|| text.to_string());
        written.push_str(&tr.replace('\n', EOL));
        written.push_str(EOL);
    };

    for (line, _) in crate::text::lines_with_breaks(raw) {
        let trimmed = line.trim();
        if !wait_text {
            if time_re.is_match(trimmed) {
                wait_text = true;
            }
            key = trimmed.to_string();
            text.clear();
            written.push_str(line);
            written.push_str(EOL);
        } else if trimmed.is_empty() {
            flush(&key, &text, &mut segments, &mut written, translations);
            written.push_str(EOL);
            wait_text = false;
            key.clear();
            text.clear();
        } else {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(line);
        }
    }
    flush(&key, &text, &mut segments, &mut written, translations);

    TimedOutcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

#[allow(dead_code)]
pub fn write_blocks(
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
