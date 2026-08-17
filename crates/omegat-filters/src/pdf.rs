//! Java `org.omegat.filters2.pdf.PdfFilter`.
//!
//! Text is taken from PDF content streams (`Tj` / `TJ`) after FlateDecode,
//! then grouped the same way as Java `PDFTextStripper` + `processFile`.

use crate::misc::seg;
use crate::{ensure_parent, Filter, FilterContext, ParsedFile, Result};
use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub struct PdfFilter;

impl Filter for PdfFilter {
    fn id(&self) -> &'static str {
        "pdf"
    }
    fn name(&self) -> &'static str {
        "PDF files"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.pdf"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&std::fs::read(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&std::fs::read(source_path)?, Some(translations)).written;
        let dest = if dest_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            dest_path.with_extension("pdf.txt")
        } else {
            dest_path.to_path_buf()
        };
        ensure_parent(&dest)?;
        std::fs::write(&dest, &out)?;
        if dest_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            std::fs::write(dest_path, &out)?;
        }
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

fn process(bytes: &[u8], translations: Option<&HashMap<String, String>>) -> Outcome {
    let lines = extract_pdf_lines(bytes);
    let mut segments = Vec::new();
    let mut written = String::new();
    let mut sb = String::new();
    for s in &lines {
        if s.trim().is_empty() {
            flush_pdf(&mut sb, translations, &mut segments, &mut written, true);
        } else {
            sb.push_str(s);
            sb.push(' ');
        }
    }
    if !sb.is_empty() {
        flush_pdf(&mut sb, translations, &mut segments, &mut written, false);
    }
    Outcome {
        parsed: ParsedFile {
            segments,
            skeleton: Some(written.clone()),
        },
        written,
    }
}

fn flush_pdf(
    sb: &mut String,
    translations: Option<&HashMap<String, String>>,
    segments: &mut Vec<crate::ExtractedSegment>,
    written: &mut String,
    blank_after: bool,
) {
    if sb.is_empty() {
        if blank_after {
            written.push_str("\n\n");
        }
        return;
    }
    let source = std::mem::take(sb);
    if !source.is_empty() {
        segments.push(seg(segments.len().to_string(), &source));
    }
    let trans = if let Some(map) = translations {
        map.get(&source).cloned().unwrap_or(source)
    } else {
        source
    };
    written.push_str(&trans);
    if blank_after {
        written.push_str("\n\n");
    } else {
        written.push('\n');
    }
}

fn extract_pdf_lines(bytes: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = find_subslice(&bytes[i..], b"stream") {
        let s = i + rel;
        let mut p = s + 6;
        if p < bytes.len() && (bytes[p] == b'\n' || bytes[p] == b'\r') {
            if bytes[p] == b'\r' && p + 1 < bytes.len() && bytes[p + 1] == b'\n' {
                p += 2;
            } else {
                p += 1;
            }
        }
        let Some(end_rel) = find_subslice(&bytes[p..], b"endstream") else {
            break;
        };
        let mut data = &bytes[p..p + end_rel];
        if let Some(d) = data.strip_suffix(b"\r\n") {
            data = d;
        } else if let Some(d) = data.strip_suffix(b"\n").or_else(|| data.strip_suffix(b"\r")) {
            data = d;
        }
        if let Some(dec) = inflate(data) {
            lines.extend(strings_from_content(&dec));
        }
        i = p + end_rel + 9;
    }
    lines
}

fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = ZlibDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).ok()?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn strings_from_content(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut lines = Vec::new();
    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some((s, end)) = parse_tj_array(&text[i..]) {
                if !s.is_empty() {
                    lines.push(s);
                }
                i += end;
                continue;
            }
        }
        if bytes[i] == b'(' {
            if let Some((s, end)) = parse_pdf_string(&text[i..]) {
                let after = text[i + end..].trim_start();
                if after.starts_with("Tj") || after.starts_with("'") || after.starts_with("\"") {
                    if !s.is_empty() {
                        lines.push(s);
                    }
                }
                i += end;
                continue;
            }
        }
        i += 1;
    }
    lines
}

fn parse_tj_array(s: &str) -> Option<(String, usize)> {
    if !s.starts_with('[') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1usize;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() {
        if chars[i] == ']' {
            let rest: String = chars[i + 1..].iter().collect();
            if rest.trim_start().starts_with("TJ") {
                return Some((out, s[..=i].len() + rest.find('J').unwrap_or(0) + 1));
            }
            return None;
        }
        if chars[i] == '(' {
            let slice: String = chars[i..].iter().collect();
            let (part, n) = parse_pdf_string(&slice)?;
            out.push_str(&part);
            i += slice[..n].chars().count();
            continue;
        }
        i += 1;
    }
    None
}

fn parse_pdf_string(s: &str) -> Option<(String, usize)> {
    if !s.starts_with('(') {
        return None;
    }
    let mut out = String::new();
    let mut depth = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            i += 1;
            match chars[i] {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '(' => out.push('('),
                ')' => out.push(')'),
                '\\' => out.push('\\'),
                other => out.push(other),
            }
            i += 1;
            continue;
        }
        if c == '(' {
            depth += 1;
            if depth > 1 {
                out.push(c);
            }
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                let byte_len = chars[..=i].iter().map(|ch| ch.len_utf8()).sum();
                return Some((out, byte_len));
            }
            out.push(c);
        } else if depth > 0 {
            out.push(c);
        }
        i += 1;
    }
    None
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
