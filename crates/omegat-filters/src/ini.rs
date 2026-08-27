//! Java `org.omegat.filters2.text.ini.INIFilter`.

use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct IniFilter;

impl Filter for IniFilter {
    fn id(&self) -> &'static str {
        "ini"
    }
    fn name(&self) -> &'static str {
        "INI / Key=Value"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.ini", "*.lng", "*.strings"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut group: Option<String> = None;
    let mut key: Option<String> = None;
    let mut contlines = 0i32;
    let mut line_no = 0usize;

    for (line, br) in crate::text::lines_with_breaks(raw) {
        line_no += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            written.push_str(line);
            written.push_str(br);
            contlines = 0;
            key = None;
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            group = Some(trimmed[1..trimmed.len() - 1].to_string());
            written.push_str(line);
            written.push_str(br);
            contlines = 0;
            key = None;
            continue;
        }

        let equals_pos = line.find('=');
        let (omegat_id, after_eq, new_key, reset_cont) = if let Some(mut eq) = equals_pos {
            let bytes = line.as_bytes();
            while eq + 1 < bytes.len() && bytes[eq + 1] == b' ' {
                eq += 1;
            }
            let k = line[..eq].to_string();
            let id = build_id(group.as_deref(), &k);
            (id, eq + 1, Some(k), true)
        } else {
            let id = if let Some(k) = &key {
                contlines += 1;
                build_virtual_id(group.as_deref(), k, contlines)
            } else {
                build_id(group.as_deref(), &format!("#L{line_no}"))
            };
            let mut after = 0usize;
            let chars: Vec<char> = line.chars().collect();
            while after < chars.len() && chars[after] == ' ' && chars.len() - after > 1 {
                after += chars[after].len_utf8();
            }
            (id, after, key.clone(), false)
        };
        if reset_cont {
            key = new_key;
            contlines = 0;
        } else {
            key = new_key;
        }

        written.push_str(&line[..after_eq.min(line.len())]);
        let mut value = left_trim(&line[after_eq.min(line.len())..]);
        let mut has_quote = false;
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
            has_quote = true;
        }
        segments.push(ExtractedSegment {
            id: omegat_id.clone(),
            source: value.clone(),
            existing_translation: None,
            note: None,
            comment: None,
            path: Some(omegat_id.clone()),
            protected_parts: vec![],
        });
        let trans = translations
            .and_then(|m| m.get(&omegat_id).or_else(|| m.get(&value)).cloned())
            .unwrap_or(value);
        if has_quote {
            written.push('"');
        }
        written.push_str(&trans);
        if has_quote {
            written.push('"');
        }
        written.push_str(br);
    }

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn left_trim(s: &str) -> String {
    s.trim_start_matches([' ', '\t']).to_string()
}

fn build_id(group: Option<&str>, key: &str) -> String {
    match group {
        Some(g) => format!("{}/{}", g, key.trim()),
        None => key.trim().to_string(),
    }
}

fn build_virtual_id(group: Option<&str>, key: &str, counter: i32) -> String {
    match group {
        Some(g) => format!("{}/{}/#{}", g, key.trim(), counter),
        None => format!("{}/#{}", key.trim(), counter),
    }
}
