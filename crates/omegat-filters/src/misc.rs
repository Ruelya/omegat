//! Shared helpers for text-like filters2 ports. Not a filter.

use crate::{placeholder, ExtractedSegment, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;

/// Java `getTranslation(id, source)`: id first, then source text, else source.
#[allow(dead_code)]
pub fn lookup_trans(
    translations: Option<&HashMap<String, String>>,
    id: &str,
    source: &str,
) -> String {
    translations
        .and_then(|m| m.get(id).cloned().or_else(|| m.get(source).cloned()))
        .unwrap_or_else(|| source.to_string())
}

pub fn seg(id: impl Into<String>, source: impl Into<String>) -> ExtractedSegment {
    let id = id.into();
    ExtractedSegment {
        id: id.clone(),
        source: source.into(),
        existing_translation: None,
        note: None,
        comment: None,
        path: Some(id),
        protected_parts: vec![],
    }
}

#[allow(dead_code)]
pub fn kv_parser(raw: &str, assign: &[char]) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            skeleton.push_str(line);
            skeleton.push('\n');
            continue;
        }
        if let Some(pos) = trimmed.find(assign) {
            let (k, v) = trimmed.split_at(pos);
            let v = v.trim_start_matches(assign).trim();
            if v.is_empty() {
                skeleton.push_str(line);
                skeleton.push('\n');
                continue;
            }
            skeleton.push_str(k);
            if trimmed[pos..].starts_with('=') {
                skeleton.push('=');
            } else {
                skeleton.push_str(&trimmed[pos..pos + 1]);
            }
            skeleton.push_str(&placeholder(segments.len()));
            skeleton.push('\n');
            segments.push(ExtractedSegment {
                id: segments.len().to_string(),
                source: v.trim_matches('"').to_string(),
                existing_translation: None,
                note: None,
                comment: None,
                path: Some(k.trim().to_string()),
                protected_parts: vec![],
            });
        } else {
            skeleton.push_str(line);
            skeleton.push('\n');
        }
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

#[allow(dead_code)]
pub fn token_replace(raw: &str, re: &Regex, group: usize) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    let mut last = 0usize;
    for cap in re.captures_iter(raw) {
        let m = cap.get(group).unwrap();
        skeleton.push_str(&raw[last..m.start()]);
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: m.as_str().to_string(),
            existing_translation: None,
            note: None,
            comment: None,
            path: cap.get(1).map(|g| g.as_str().to_string()),
            protected_parts: vec![],
        });
        last = m.end();
    }
    skeleton.push_str(&raw[last..]);
    Ok(ParsedFile {
        segments,
        skeleton: Some(skeleton),
    })
}

#[allow(dead_code)]
pub fn paragraphs(raw: &str) -> Result<ParsedFile> {
    let normalized = raw.replace("\r\n", "\n");
    let mut segments = Vec::new();
    let mut skeleton = String::new();
    for (i, part) in normalized.split("\n\n").enumerate() {
        if i > 0 {
            skeleton.push_str("\n\n");
        }
        if part.trim().is_empty() {
            skeleton.push_str(part);
            continue;
        }
        skeleton.push_str(&placeholder(segments.len()));
        segments.push(ExtractedSegment {
            id: segments.len().to_string(),
            source: part.trim().to_string(),
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
