//! Java `org.omegat.filters2.text.dokuwiki.DokuWikiFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct DokuWikiFilter;

impl Filter for DokuWikiFilter {
    fn id(&self) -> &'static str {
        "dokuwiki"
    }
    fn name(&self) -> &'static str {
        "DokuWiki"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.txt"]
    }
    fn file_supported(&self, path: &Path, _ctx: &FilterContext) -> bool {
        read_to_string(path)
            .map(|raw| raw.lines().any(|l| heading_level(l.trim()) > 0))
            .unwrap_or(false)
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
    let code_tag = Regex::new(r"<code|<file|<html|<php|/\*").unwrap();
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut text = String::new();
    let lines = crate::text::lines_with_breaks(raw);
    let mut i = 0usize;
    let mut last_br = "\n";

    while i < lines.len() {
        let (line, br) = lines[i];
        last_br = br;
        i += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));
            written.push_str(line);
            written.push_str(br);
            continue;
        }
        let heading = heading_level(trimmed);
        if heading > 0 {
            flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));
            let header = trimmed[heading..trimmed.len() - heading].trim();
            let mut out_line = line.to_string();
            if !header.is_empty() {
                let trans = take(header, translations, &mut segments);
                out_line = out_line.replacen(header, &trans, 1);
            }
            written.push_str(&out_line);
            written.push_str(br);
            continue;
        }
        if line.starts_with("  *") || line.starts_with("  -") {
            flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));
            written.push_str(&line[..3]);
            written.push(' ');
            write_value(&line[3..], translations, &mut segments, &mut written, Some(last_br));
            continue;
        }
        if (trimmed.starts_with("{{") && trimmed.ends_with("}}"))
            || (trimmed.starts_with("~~") && trimmed.ends_with("~~") && trimmed.len() > 5)
        {
            flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));
            written.push_str(line);
            written.push_str(br);
            continue;
        }
        if line.starts_with('|') || line.starts_with('^') {
            flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));
            let mut start = 0usize;
            let mut brace = 0i32;
            for (byte_i, cp) in line.char_indices() {
                match cp {
                    '|' | '^' => {
                        if brace == 0 {
                            if start > 0 {
                                written.push(' ');
                                write_value(
                                    &line[start..byte_i],
                                    translations,
                                    &mut segments,
                                    &mut written,
                                    None,
                                );
                                written.push(' ');
                            }
                            written.push(cp);
                            start = byte_i + cp.len_utf8();
                        }
                    }
                    '{' => brace += 1,
                    '}' => brace -= 1,
                    _ => {}
                }
            }
            written.push_str(br);
            continue;
        }
        match skip_code(
            &code_tag,
            line,
            br,
            &lines,
            &mut i,
            &mut text,
            translations,
            &mut segments,
            &mut written,
        ) {
            None => break,
            Some(rest) => {
                text.push(' ');
                text.push_str(rest.trim());
            }
        }
    }
    flush_text(&mut text, translations, &mut segments, &mut written, Some(last_br));

    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn heading_level(line: &str) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut start = 0usize;
    let mut end = chars.len();
    let mut level = 0usize;
    while start < end {
        if chars[start] != '=' || chars[end - 1] != '=' {
            break;
        }
        start += 1;
        end -= 1;
        level += 1;
    }
    if start < end && (end - start) > 1 {
        level
    } else {
        0
    }
}

fn collapse_spaces(mut value: String) -> String {
    loop {
        let next = value.replace("  ", " ");
        if next == value {
            return value;
        }
        value = next;
    }
}

fn take(
    value: &str,
    translations: Option<&HashMap<String, String>>,
    segments: &mut Vec<crate::ExtractedSegment>,
) -> String {
    let value = collapse_spaces(value.trim().to_string());
    if value.is_empty() {
        return String::new();
    }
    segments.push(seg(segments.len().to_string(), &value));
    if let Some(map) = translations {
        map.get(&value).cloned().unwrap_or(value)
    } else {
        value
    }
}

fn write_value(
    value: &str,
    translations: Option<&HashMap<String, String>>,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
    br: Option<&str>,
) {
    let trans = take(value, translations, segments);
    if !trans.is_empty() {
        written.push_str(&trans);
        if let Some(b) = br {
            written.push_str(b);
        }
    }
}

fn flush_text(
    text: &mut String,
    translations: Option<&HashMap<String, String>>,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
    br: Option<&str>,
) {
    if text.is_empty() {
        return;
    }
    let value = std::mem::take(text);
    write_value(&value, translations, segments, written, br);
}

fn skip_code<'a>(
    code_tag: &Regex,
    line: &'a str,
    br: &'a str,
    lines: &'a [(&'a str, &'a str)],
    i: &mut usize,
    text: &mut String,
    translations: Option<&HashMap<String, String>>,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
) -> Option<String> {
    let mut owned = line.to_string();
    loop {
        let Some(m) = code_tag.find(&owned) else {
            return Some(owned);
        };
        let start = m.start();
        let tag_name = owned[start + 1..m.end()].to_string();
        let is_asterisk = tag_name == "*";
        text.push(' ');
        text.push_str(&owned[..start]);
        if !is_asterisk {
            flush_text(text, translations, segments, written, Some(br));
        }
        let end_pat = if is_asterisk {
            r"\*/".to_string()
        } else {
            format!(r"</{}>", regex::escape(&tag_name))
        };
        let end_tag = Regex::new(&end_pat).unwrap();
        owned = owned[start..].to_string();
        loop {
            if let Some(em) = end_tag.find(&owned) {
                written.push_str(&owned[..em.end()]);
                written.push_str(br);
                owned = owned[em.end()..].to_string();
                break;
            }
            written.push_str(&owned);
            written.push_str(br);
            if *i >= lines.len() {
                return None;
            }
            owned = lines[*i].0.to_string();
            *i += 1;
        }
        let _ = line;
    }
}
