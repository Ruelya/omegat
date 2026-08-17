use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct PropertiesFilter;

impl Filter for PropertiesFilter {
    fn id(&self) -> &'static str {
        "properties"
    }
    fn name(&self) -> &'static str {
        "Java Resource Bundles"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.properties"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        parse_props(&read_to_string(path)?)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let mut out = String::new();
        for line in raw.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                out.push_str(line);
                out.push('\n');
                continue;
            }
            if let Some((k, _)) = split_kv(line) {
                if let Some(t) = translations.get(k) {
                    out.push_str(k);
                    out.push('=');
                    out.push_str(&escape_prop(t));
                    out.push('\n');
                    continue;
                }
            }
            out.push_str(line);
            out.push('\n');
        }
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

fn parse_props(raw: &str) -> Result<ParsedFile> {
    let mut segments = Vec::new();
    let mut pending_comment = String::new();
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with('!') {
            pending_comment.push_str(trimmed.trim_start_matches(['#', '!']).trim());
            pending_comment.push('\n');
            continue;
        }
        if trimmed.is_empty() {
            pending_comment.clear();
            continue;
        }
        if let Some((k, v)) = split_kv(line) {
            segments.push(ExtractedSegment {
                id: k.to_string(),
                source: unescape_prop(v),
                existing_translation: None,
                note: None,
                comment: if pending_comment.is_empty() {
                    None
                } else {
                    Some(pending_comment.trim().to_string())
                },
                path: Some(k.to_string()),
                protected_parts: vec![],
            });
        }
        pending_comment.clear();
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    let eq = line.find('=').or_else(|| line.find(':'))?;
    Some((line[..eq].trim(), line[eq + 1..].trim()))
}

fn unescape_prop(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(v) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(v) {
                            out.push(ch);
                            continue;
                        }
                    }
                    out.push_str("\\u");
                    out.push_str(&hex);
                }
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape_prop(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t")
}
