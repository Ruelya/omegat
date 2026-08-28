//! Plain-text filter. Line/paragraph rules follow Java `TextFilter`.

use crate::{
    apply_skeleton_with_originals, ensure_parent, merge_translations, placeholder, read_to_string,
    read_to_string_cancellable, ExtractedSegment, Filter, FilterContext, FilterError, ParsedFile,
    Result,
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
    fn parse_cancellable(
        &self,
        path: &Path,
        ctx: &FilterContext,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ParsedFile> {
        let raw = read_to_string_cancellable(path, is_cancelled)?;
        let parsed = parse_text(&raw, ctx.option("segmentOn").unwrap_or("EMPTYLINES"))?;
        if is_cancelled() {
            Err(FilterError::Cancelled)
        } else {
            Ok(parsed)
        }
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
        let out = apply_line_length_limit(&out, ctx);
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

pub(crate) fn lines_with_breaks(raw: &str) -> Vec<(&str, &str)> {
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

/// Java `LineLengthLimitWriter` wrapping the reconstructed target file.
/// Java `LineLengthLimitWriter` product entry (also used by goldens).
pub fn apply_line_length_limit(text: &str, ctx: &FilterContext) -> String {
    let line_length = ctx
        .option("lineLength")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let max_line_length = ctx
        .option("maxLineLength")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    if line_length == 0 || max_line_length == 0 {
        return text.to_string();
    }
    let mut w = LineLengthLimitWriter::new(line_length, max_line_length);
    w.write_str(text);
    w.close()
}

struct Tok {
    offset: usize,
    length: usize,
}

pub struct LineLengthLimitWriter {
    out: String,
    line_length: i32,
    max_line_length: i32,
    buf: Vec<char>,
    break_chars: i32,
    eol1: char,
    eol2: char,
}

impl LineLengthLimitWriter {
    pub fn wrap(text: &str, line_length: i32, max_line_length: i32) -> String {
        let mut w = Self::new(line_length, max_line_length);
        w.write_str(text);
        w.close()
    }

    pub fn is_spaces_slice(chars: &[char]) -> bool {
        !chars.is_empty() && chars.iter().all(|c| c.is_whitespace())
    }

    pub fn is_spaces_token(buf: &str, offset: usize, length: usize) -> bool {
        let chars: Vec<char> = buf.chars().collect();
        if offset + length > chars.len() {
            return false;
        }
        Self::is_spaces_slice(&chars[offset..offset + length])
    }

    pub fn is_possible_break_before_in(buf: &str, pos: usize) -> bool {
        let chars: Vec<char> = buf.chars().collect();
        let w = Self::new(80, 100);
        let mut probe = w;
        probe.buf = chars;
        probe.possible_break_before(pos)
    }

    pub fn break_pos(buf: &str, line_length: i32, max_line_length: i32) -> usize {
        let mut w = Self::new(line_length, max_line_length);
        w.buf = buf.chars().collect();
        let tokens = tokenize_verbatim(&w.buf);
        w.get_break_pos(&tokens)
    }

    pub fn new(line_length: i32, max_line_length: i32) -> Self {
        Self {
            out: String::new(),
            line_length,
            max_line_length,
            buf: Vec::new(),
            break_chars: 0,
            eol1: '\0',
            eol2: '\0',
        }
    }

    fn write_str(&mut self, s: &str) {
        for cp in s.chars() {
            if self.break_chars > 0 && !self.buf.is_empty() && cp == *self.buf.last().unwrap() {
                self.out_line();
            }
            if cp == '\n' || cp == '\r' {
                self.buf.push(cp);
                self.break_chars += 1;
                if self.break_chars > 1 {
                    self.out_line();
                }
            } else {
                if self.break_chars > 0 {
                    self.out_line();
                }
                self.buf.push(cp);
            }
        }
    }

    fn close(mut self) -> String {
        self.out_line();
        self.out
    }

    fn out_line(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let mut cp = self.buf[self.buf.len() - 1];
        if cp == '\n' || cp == '\r' {
            self.eol2 = cp;
            self.buf.pop();
        }
        if !self.buf.is_empty() {
            cp = self.buf[self.buf.len() - 1];
            if cp == '\n' || cp == '\r' {
                self.eol1 = cp;
                self.buf.pop();
            }
        }
        if self.buf.is_empty() {
            self.write_source_eol();
        } else {
            let mut tokens = tokenize_verbatim(&self.buf);
            while !self.buf.is_empty() {
                let p = self.get_break_pos(&tokens);
                self.break_at(p, &mut tokens);
            }
        }
        self.break_chars = 0;
        self.eol1 = '\0';
        self.eol2 = '\0';
    }

    fn cp_count(&self, end: usize) -> i32 {
        end.min(self.buf.len()) as i32
    }

    fn get_break_pos(&self, tokens: &[Option<Tok>]) -> usize {
        if self.cp_count(self.buf.len()) <= self.max_line_length {
            return self.buf.len();
        }
        let mut latest_non_spaces = 0usize;
        for t in tokens.iter().rev().flatten() {
            if self.is_spaces(t) {
                continue;
            }
            latest_non_spaces = t.offset + t.length;
            break;
        }
        if self.cp_count(latest_non_spaces) <= self.max_line_length {
            return self.buf.len();
        }
        let mut spaces_start: i32 = -1;
        for t in tokens.iter().flatten() {
            if self.cp_count(t.offset) >= self.line_length
                && spaces_start >= 0
                && self.cp_count(spaces_start as usize) < self.max_line_length
            {
                return t.offset;
            }
            if self.is_spaces(t) {
                if spaces_start < 0 {
                    spaces_start = t.offset as i32;
                }
            } else {
                spaces_start = -1;
            }
        }
        for t in tokens.iter().flatten() {
            let cps = self.cp_count(t.offset);
            if cps >= self.line_length && cps < self.max_line_length && self.is_spaces(t) {
                return t.offset;
            }
            let cps = self.cp_count(t.offset + t.length);
            if cps >= self.line_length && cps < self.max_line_length && self.is_spaces(t) {
                return t.offset + t.length;
            }
        }
        for t in tokens.iter().flatten() {
            let cps = self.cp_count(t.offset);
            if cps >= self.line_length && cps < self.max_line_length && self.possible_break_before(t.offset)
            {
                return t.offset;
            }
            let cps = self.cp_count(t.offset + t.length);
            if cps >= self.line_length
                && cps < self.max_line_length
                && self.possible_break_before(t.offset + t.length)
            {
                return t.offset + t.length;
            }
        }
        for (i, t) in tokens.iter().enumerate() {
            let Some(t) = t else { continue };
            if self.cp_count(t.offset) >= self.line_length {
                if i == 0 {
                    return t.offset + t.length;
                }
                let mut j = i as i32 - 1;
                while j >= 0 {
                    if let Some(tp) = tokens[j as usize].as_ref() {
                        if tp.offset > 0 && self.possible_break_before(tp.offset) {
                            return tp.offset;
                        }
                    }
                    j -= 1;
                }
                return t.offset;
            }
        }
        self.buf.len()
    }

    fn is_spaces(&self, token: &Tok) -> bool {
        self.buf[token.offset..token.offset + token.length]
            .iter()
            .all(|c| c.is_whitespace())
    }

    fn break_at(&mut self, pos: usize, tokens: &mut [Option<Tok>]) {
        let pos = pos.min(self.buf.len());
        let head: String = rstrip_chars(&self.buf[..pos]);
        self.out.push_str(&head);
        self.buf.drain(..pos);
        if !self.buf.is_empty() {
            self.write_break_eol();
        } else {
            self.write_source_eol();
        }
        for t in tokens.iter_mut() {
            match t {
                Some(tok) if tok.offset < pos => *t = None,
                Some(tok) => tok.offset -= pos,
                None => {}
            }
        }
    }

    fn write_break_eol(&mut self) {
        if self.eol1 == '\0' && self.eol2 == '\0' {
            self.out.push('\n');
        } else {
            if self.eol1 != '\0' {
                self.out.push(self.eol1);
            }
            if self.eol2 != '\0' {
                self.out.push(self.eol2);
            }
        }
    }

    fn write_source_eol(&mut self) {
        if self.eol1 != '\0' {
            self.out.push(self.eol1);
        }
        if self.eol2 != '\0' {
            self.out.push(self.eol2);
        }
    }

    fn possible_break_before(&self, pos: usize) -> bool {
        if pos > 0 && pos <= self.buf.len() {
            let cp = self.buf[pos - 1];
            if ":\\([{<\u{00ab}\u{201e}".contains(cp) {
                return false;
            }
        }
        if pos < self.buf.len() {
            let cp = self.buf[pos];
            if "{:)]}>\u{00bb}\u{201c},.".contains(cp) {
                return false;
            }
        }
        true
    }
}

fn rstrip_chars(chars: &[char]) -> String {
    let mut end = chars.len();
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    chars[..end].iter().collect()
}

/// Word/whitespace/other groups, matching Java `BreakIterator.getWordInstance` on these fixtures.
fn tokenize_verbatim(chars: &[char]) -> Vec<Option<Tok>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let start = i;
        let c = chars[i];
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else if c.is_alphanumeric() {
            while i < chars.len() && chars[i].is_alphanumeric() {
                i += 1;
            }
        } else {
            i += 1;
        }
        out.push(Some(Tok {
            offset: start,
            length: i - start,
        }));
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
