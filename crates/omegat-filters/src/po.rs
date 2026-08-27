//! GNU gettext PO filter. Parse/write follow Java `PoFilter` line state machine.

use crate::{
    ensure_parent, extract_tags, ExtractedSegment, Filter, FilterContext, ParsedFile,
    ProtectedPart, Result,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct PoFilter;

impl Filter for PoFilter {
    fn id(&self) -> &'static str {
        "po"
    }
    fn name(&self) -> &'static str {
        "PO"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.po", "*.pot"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let raw = read_po_bytes(path)?;
        Ok(process_po(&raw, ctx, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_po_bytes(source_path)?;
        let out = process_po(&raw, ctx, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

/// Java `InputStreamReader(UTF-8)` + `CodingErrorAction.REPLACE`: one U+FFFD
/// per malformed sequence (`malformed(n)`), not WHATWG-per-byte replacement.
fn read_po_bytes(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode_utf8_java_replace(&bytes))
}

fn decode_utf8_java_replace(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    let sl = bytes.len();
    while i < sl {
        let b1 = bytes[i] as i8;
        if b1 >= 0 {
            out.push(bytes[i] as char);
            i += 1;
        } else if (b1 >> 5) == -2 && (b1 & 0x1e) != 0 {
            if sl - i < 2 {
                out.push('\u{FFFD}');
                break;
            }
            let b2 = bytes[i + 1];
            if b2 & 0xc0 != 0x80 {
                out.push('\u{FFFD}');
                i += 1;
            } else {
                let cp = (((b1 as u8 as u32) & 0x1f) << 6) | (b2 as u32 & 0x3f);
                out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                i += 2;
            }
        } else if (b1 >> 4) == -2 {
            let rem = sl - i;
            if rem < 3 {
                if rem > 1 && is_malformed3_2(b1, bytes[i + 1] as i8) {
                    out.push('\u{FFFD}');
                    i += 1;
                    continue;
                }
                out.push('\u{FFFD}');
                break;
            }
            let b2 = bytes[i + 1] as i8;
            let b3 = bytes[i + 2] as i8;
            if is_malformed3(b1, b2, b3) {
                out.push('\u{FFFD}');
                i += malformed3_len(b1, b2);
                continue;
            }
            let cp = (((b1 as u8 as u32) & 0x0f) << 12)
                | (((b2 as u8 as u32) & 0x3f) << 6)
                | (b3 as u8 as u32 & 0x3f);
            if (0xd800..=0xdfff).contains(&cp) {
                out.push('\u{FFFD}');
                i += 3;
                continue;
            }
            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
            i += 3;
        } else if (b1 >> 3) == -2 {
            let rem = sl - i;
            let lead = bytes[i];
            if rem < 4 {
                if lead > 0xf4 || (rem > 1 && is_malformed4_2(lead, bytes[i + 1])) {
                    out.push('\u{FFFD}');
                    i += 1;
                    continue;
                }
                if rem > 2 && bytes[i + 2] & 0xc0 != 0x80 {
                    out.push('\u{FFFD}');
                    i += 2;
                    continue;
                }
                out.push('\u{FFFD}');
                break;
            }
            let b2 = bytes[i + 1];
            let b3 = bytes[i + 2];
            let b4 = bytes[i + 3];
            let uc = (((lead as u32) & 0x07) << 18)
                | ((b2 as u32 & 0x3f) << 12)
                | ((b3 as u32 & 0x3f) << 6)
                | (b4 as u32 & 0x3f);
            if b2 & 0xc0 != 0x80
                || b3 & 0xc0 != 0x80
                || b4 & 0xc0 != 0x80
                || !(0x10000..=0x10ffff).contains(&uc)
            {
                out.push('\u{FFFD}');
                i += malformed4_len(lead, b2, b3);
                continue;
            }
            out.push(char::from_u32(uc).unwrap_or('\u{FFFD}'));
            i += 4;
        } else {
            out.push('\u{FFFD}');
            i += 1;
        }
    }
    out
}

fn is_malformed3(b1: i8, b2: i8, b3: i8) -> bool {
    (b1 == 0xe0u8 as i8 && (b2 as u8 & 0xe0) == 0x80)
        || (b2 as u8 & 0xc0) != 0x80
        || (b3 as u8 & 0xc0) != 0x80
}

fn is_malformed3_2(b1: i8, b2: i8) -> bool {
    (b1 == 0xe0u8 as i8 && (b2 as u8 & 0xe0) == 0x80) || (b2 as u8 & 0xc0) != 0x80
}

fn malformed3_len(b1: i8, b2: i8) -> usize {
    if (b1 == 0xe0u8 as i8 && (b2 as u8 & 0xe0) == 0x80) || (b2 as u8 & 0xc0) != 0x80 {
        1
    } else {
        2
    }
}

fn is_malformed4_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
        || (b1 == 0xf4 && b2 & 0xf0 != 0x80)
        || b2 & 0xc0 != 0x80
}

fn malformed4_len(b1: u8, b2: u8, b3: u8) -> usize {
    if b1 > 0xf4
        || (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
        || (b1 == 0xf4 && b2 & 0xf0 != 0x80)
        || b2 & 0xc0 != 0x80
    {
        1
    } else if b3 & 0xc0 != 0x80 {
        2
    } else {
        3
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Msgid,
    MsgidPlural,
    Msgstr,
    MsgstrPlural,
    Msgctx,
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

struct Engine<'a> {
    allow_blank: bool,
    allow_editing_blank: bool,
    skip_header: bool,
    auto_fill_plural: bool,
    monolingual: bool,
    target_lang: String,
    translations: Option<&'a HashMap<String, String>>,
    writing: bool,
    out: String,
    segments: Vec<ExtractedSegment>,
    sources: [String; 2],
    targets: Vec<String>,
    translator_comments: String,
    extracted_comments: String,
    references: String,
    source_fuzzy_true: String,
    plurals: usize,
    path: String,
    nowrap: bool,
    fuzzy: bool,
    fuzzy_true: bool,
    header_processed: bool,
    current_mode: Option<Mode>,
    current_plural: usize,
}

fn option_true_default(ctx: &FilterContext, key: &str) -> bool {
    match ctx.option(key) {
        None => true,
        Some(s) => s.eq_ignore_ascii_case("true"),
    }
}

fn process_po(
    raw: &str,
    ctx: &FilterContext,
    translations: Option<&HashMap<String, String>>,
) -> Outcome {
    let mut eng = Engine {
        allow_blank: option_true_default(ctx, "disallowBlank"),
        allow_editing_blank: option_true_default(ctx, "allowEditingBlankSegment"),
        skip_header: ctx.option_flag("skipHeader"),
        auto_fill_plural: ctx.option_flag("autoFillInPluralStatement"),
        monolingual: ctx.option_flag("monolingualFormat"),
        target_lang: ctx.target_lang.clone(),
        translations,
        writing: translations.is_some(),
        out: String::new(),
        segments: Vec::new(),
        sources: [String::new(), String::new()],
        targets: vec![String::new(), String::new()],
        translator_comments: String::new(),
        extracted_comments: String::new(),
        references: String::new(),
        source_fuzzy_true: String::new(),
        plurals: 2,
        path: String::new(),
        nowrap: false,
        fuzzy: false,
        fuzzy_true: false,
        header_processed: false,
        current_mode: None,
        current_plural: 0,
    };
    for line in raw.lines() {
        let s = java_trim(line);
        if eng.process_fuzzy(s) {
            continue;
        }
        if eng.process_fuzzy_markers(s) {
            continue;
        }
        if COMMENT_FUZZY_OTHER.is_match(s) {
            eng.current_plural = 0;
            eng.fuzzy = true;
            eng.flush();
            let stripped = COMMENT_FUZZY_STRIP.replace(s, "$1$2").into_owned();
            eng.dispatch_rest(&stripped);
            continue;
        }
        eng.dispatch_rest(s);
    }
    eng.flush();
    Outcome {
        parsed: ParsedFile {
            segments: eng.segments,
            skeleton: Some(raw.to_string()),
        },
        written: eng.out,
    }
}

impl Engine<'_> {
    fn dispatch_rest(&mut self, s: &str) {
        if self.process_nowrap(s) {
            return;
        }
        if self.process_msgid(s) {
            return;
        }
        if self.process_msgstr(s) {
            return;
        }
        if self.process_msgctxt(s) {
            return;
        }
        if self.process_comments(s) {
            return;
        }
        if self.process_fuzzy_message(s) {
            return;
        }
        if self.process_other(s) {
            return;
        }
        self.flush();
        self.eol(s);
    }

    fn process_fuzzy(&mut self, line: &str) -> bool {
        if let Some(c) = COMMENT_FUZZY_MSGID.captures(line) {
            self.fuzzy_true = true;
            self.source_fuzzy_true.push_str(&c[1]);
            return true;
        }
        COMMENT_FUZZY_MSGCTX.is_match(line)
    }

    fn process_fuzzy_markers(&mut self, line: &str) -> bool {
        if COMMENT_FUZZY.is_match(line) {
            self.current_plural = 0;
            self.fuzzy = true;
            self.flush();
            return true;
        }
        false
    }

    fn process_nowrap(&mut self, line: &str) -> bool {
        if COMMENT_NOWRAP.is_match(line) {
            self.current_plural = 0;
            self.flush();
            self.nowrap = true;
            self.eol(line);
            return true;
        }
        false
    }

    fn process_msgid(&mut self, line: &str) -> bool {
        let Some(c) = MSG_ID.captures(line) else {
            return false;
        };
        self.current_plural = 0;
        let text = c.get(2).map(|m| m.as_str()).unwrap_or("");
        if c.get(1).is_none() {
            if !self.sources[0].is_empty() {
                self.flush();
            }
            self.current_mode = Some(Mode::Msgid);
            self.sources[0].push_str(text);
        } else {
            self.current_mode = Some(Mode::MsgidPlural);
            self.sources[1].push_str(text);
        }
        self.eol(line);
        true
    }

    fn process_msgstr(&mut self, line: &str) -> bool {
        let Some(c) = MSG_STR.captures(line) else {
            return false;
        };
        if self.allow_editing_blank
            && self.sources[0].is_empty()
            && !self.references.is_empty()
            && self.header_processed
        {
            let aux = format!("{}{}", self.references, self.extracted_comments);
            self.sources[0].push_str(&aux);
        }
        let text = c.get(3).map(|m| m.as_str()).unwrap_or("");
        if c.get(1).is_none() {
            self.current_mode = Some(Mode::Msgstr);
            self.targets[0].push_str(text);
            self.current_plural = 0;
        } else {
            self.current_mode = Some(Mode::MsgstrPlural);
            self.current_plural = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            if self.current_plural < self.plurals {
                while self.targets.len() <= self.current_plural {
                    self.targets.push(String::new());
                }
                self.targets[self.current_plural].push_str(text);
            }
        }
        true
    }

    fn process_msgctxt(&mut self, line: &str) -> bool {
        let Some(c) = MSG_CTX.captures(line) else {
            return false;
        };
        self.current_mode = Some(Mode::Msgctx);
        self.current_plural = 0;
        self.path = c.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
        self.eol(line);
        true
    }

    fn process_comments(&mut self, line: &str) -> bool {
        if let Some(c) = COMMENT_REFERENCE.captures(line) {
            self.current_plural = 0;
            self.references.push_str(&c[1]);
            self.references.push('\n');
            self.eol(line);
            return true;
        }
        if let Some(c) = COMMENT_EXTRACTED.captures(line) {
            self.current_plural = 0;
            self.extracted_comments.push_str(&c[1]);
            self.extracted_comments.push('\n');
            self.eol(line);
            return true;
        }
        if let Some(c) = COMMENT_TRANSLATOR.captures(line) {
            self.current_plural = 0;
            self.translator_comments.push_str(&c[1]);
            self.translator_comments.push('\n');
            self.eol(line);
            return true;
        }
        false
    }

    fn process_fuzzy_message(&mut self, line: &str) -> bool {
        if let Some(c) = MSG_FUZZY.captures(line) {
            self.source_fuzzy_true.push_str(&c[1]);
            return true;
        }
        false
    }

    fn process_other(&mut self, s: &str) -> bool {
        let Some(c) = MSG_OTHER.captures(s) else {
            return false;
        };
        let text = &c[1];
        match self.current_mode {
            None => return false,
            Some(Mode::Msgid) => {
                self.sources[0].push_str(text);
                self.eol(s);
            }
            Some(Mode::MsgidPlural) => {
                self.sources[1].push_str(text);
                self.eol(s);
            }
            Some(Mode::Msgstr) => self.targets[0].push_str(text),
            Some(Mode::MsgstrPlural) => {
                if self.current_plural < self.targets.len() {
                    self.targets[self.current_plural].push_str(text);
                }
            }
            Some(Mode::Msgctx) => {
                self.path.push_str(text);
                self.eol(s);
            }
        }
        true
    }

    fn eol(&mut self, s: &str) {
        if self.writing {
            self.out.push_str(s);
            self.out.push('\n');
        }
    }

    fn flush(&mut self) {
        if self.sources[0].is_empty() && self.path.is_empty() {
            self.header_processed = true;
            if self.targets[0].is_empty() {
                return;
            }
            let header = self.targets[0].clone();
            if let Some(c) = PLURAL_FORMS.captures(&header) {
                if let Ok(n) = c[1].parse::<usize>() {
                    self.plurals = n.max(1);
                }
            } else if let Some((n, _)) = plural_info(&self.target_lang) {
                self.plurals = n;
            }
            let first = std::mem::take(&mut self.targets[0]);
            self.targets = vec![String::new(); self.plurals];
            self.targets[0] = first;
            if self.writing {
                let quoted = self.format_translation(None, &self.targets[0], false, true, 0);
                self.out.push_str("msgstr ");
                self.out.push_str(&quoted);
                self.out.push('\n');
            } else if !self.skip_header {
                let mut header = unescape(&self.targets[0]);
                header = self.auto_fill_in_plural(&header);
                let path = self.path.clone();
                self.push_seg("", &header, None, None, &path);
            }
            self.fuzzy = false;
        } else if self.sources[1].is_empty() {
            if self.writing {
                let quoted = if self.monolingual {
                    self.format_translation(
                        Some(&self.sources[0]),
                        &self.targets[0],
                        self.allow_blank,
                        false,
                        0,
                    )
                } else {
                    self.format_translation(None, &self.sources[0], self.allow_blank, false, 0)
                };
                self.out.push_str("msgstr ");
                self.out.push_str(&quoted);
                self.out.push('\n');
            } else {
                self.parse_or_align(0);
            }
            self.fuzzy = false;
        } else {
            if self.writing {
                let q0 =
                    self.format_translation(None, &self.sources[0], self.allow_blank, false, 0);
                self.out.push_str("msgstr[0] ");
                self.out.push_str(&q0);
                self.out.push('\n');
                for i in 1..self.plurals {
                    let q =
                        self.format_translation(None, &self.sources[1], self.allow_blank, false, i);
                    self.out.push_str(&format!("msgstr[{i}] "));
                    self.out.push_str(&q);
                    self.out.push('\n');
                }
            } else {
                self.parse_or_align(0);
                for i in 1..self.plurals {
                    self.parse_or_align(i);
                }
            }
            self.fuzzy = false;
        }
        self.sources[0].clear();
        self.sources[1].clear();
        for t in &mut self.targets {
            t.clear();
        }
        self.path.clear();
        self.translator_comments.clear();
        self.extracted_comments.clear();
        self.references.clear();
        self.source_fuzzy_true.clear();
    }

    fn parse_or_align(&mut self, pair: usize) {
        let (source_raw, path_suffix) = if pair > 0 {
            (self.sources[1].clone(), format!("[{pair}]"))
        } else {
            (self.sources[0].clone(), String::new())
        };
        let source = unescape(&source_raw);
        let mut translation = unescape(self.targets.get(pair).map(|s| s.as_str()).unwrap_or(""));
        if translation.is_empty() {
            // Java sets translation = null when empty
        }
        let omt_path = format!("{}{path_suffix}", self.path);
        if self.monolingual {
            self.push_seg(&source, &translation, None, None, &omt_path);
            return;
        }
        let comments = self.build_comments(pair);
        if self.fuzzy_true {
            let fuzzy_src = self.source_fuzzy_true.clone();
            let existing = if translation.is_empty() {
                None
            } else {
                Some(translation.clone())
            };
            self.push_seg("", &fuzzy_src, existing, comments.clone(), &omt_path);
            self.fuzzy_true = false;
            self.fuzzy = false;
            translation.clear();
        }
        let existing = if translation.is_empty() {
            None
        } else {
            Some(translation)
        };
        self.push_seg("", &source, existing, comments, &omt_path);
    }

    fn build_comments(&self, pair: usize) -> Option<String> {
        let mut sb = String::new();
        if pair > 0 {
            sb.push_str(&format!("Plural form {pair}\n"));
        } else {
            let s1 = unescape(&self.sources[1]);
            if !s1.is_empty() {
                sb.push_str("Singular\n");
                sb.push_str(&s1);
                sb.push_str("\n\n");
            }
        }
        if !self.translator_comments.is_empty() {
            sb.push_str("Translator comments\n");
            sb.push_str(&unescape(&self.translator_comments));
            sb.push('\n');
        }
        if !self.extracted_comments.is_empty() {
            sb.push_str("Extracted comments\n");
            sb.push_str(&unescape(&self.extracted_comments));
            sb.push('\n');
        }
        if !self.references.is_empty() {
            sb.push_str("References\n");
            sb.push_str(&unescape(&self.references));
            sb.push('\n');
        }
        if sb.is_empty() {
            None
        } else {
            Some(sb)
        }
    }

    fn push_seg(
        &mut self,
        id: &str,
        source: &str,
        existing: Option<String>,
        comment: Option<String>,
        path: &str,
    ) {
        let tags = extract_tags(source);
        self.segments.push(ExtractedSegment {
            id: id.to_string(),
            source: source.to_string(),
            existing_translation: existing,
            note: None,
            comment,
            path: if path.is_empty() {
                None
            } else {
                Some(path.to_string())
            },
            protected_parts: tags
                .into_iter()
                .map(|t| ProtectedPart {
                    text: t,
                    details: "tag".into(),
                })
                .collect(),
        });
    }

    fn format_translation(
        &self,
        id: Option<&str>,
        en: &str,
        allow_null: bool,
        is_header: bool,
        plural: usize,
    ) -> String {
        let mut entry = unescape(en);
        let path_suffix = if plural > 0 {
            format!("[{plural}]")
        } else {
            String::new()
        };
        if is_header {
            entry = self.auto_fill_in_plural(&entry);
        }
        let mut translation = if is_header && self.skip_header {
            Some(entry.clone())
        } else {
            self.lookup(id, &entry, &format!("{}{path_suffix}", self.path))
        };
        if translation.is_none() && !allow_null {
            translation = Some(entry);
        }
        match translation {
            Some(t) => format!("\"{}\"", escape(&t, self.nowrap)),
            None => "\"\"".into(),
        }
    }

    fn lookup(&self, id: Option<&str>, entry: &str, path: &str) -> Option<String> {
        let map = self.translations?;
        if let Some(id) = id {
            if let Some(t) = map.get(id) {
                return Some(t.clone());
            }
        }
        map.get(entry)
            .cloned()
            .or_else(|| {
                if path.is_empty() {
                    None
                } else {
                    map.get(path).cloned()
                }
            })
            .or_else(|| map.get(&format!("{entry}\u{0001}{path}")).cloned())
    }

    fn auto_fill_in_plural(&self, header: &str) -> String {
        if !self.auto_fill_plural {
            return header.to_string();
        }
        let lang = language_code(&self.target_lang);
        if let Some((n, expr)) = plural_info(lang) {
            header.replace(
                "Plural-Forms: nplurals=INTEGER; plural=EXPRESSION;",
                &format!("Plural-Forms: nplurals={n}; plural={expr};"),
            )
        } else {
            header.to_string()
        }
    }
}

fn language_code(lang: &str) -> &str {
    lang.split(['-', '_']).next().unwrap_or(lang)
}

fn java_trim(s: &str) -> &str {
    let b = s.as_bytes();
    let mut start = 0;
    let mut end = b.len();
    while start < end && b[start] <= b' ' {
        start += 1;
    }
    while end > start && b[end - 1] <= b' ' {
        end -= 1;
    }
    &s[start..end]
}

/// Java `PoFilter.unescape`: R1/R2/R3 require an unescaped `\` before `"`, `n`, or `t`.
fn unescape(entry: &str) -> String {
    let entry = replace_unescaped(entry, '"', "\"");
    let entry = replace_unescaped(&entry, 'n', "\n");
    let entry = replace_unescaped(&entry, 't', "\t");
    let entry = if entry.starts_with("\\n") {
        format!("\n{}", &entry[2..])
    } else {
        entry
    };
    entry.replace("\\\\", "\\")
}

fn replace_unescaped(entry: &str, esc: char, repl: &str) -> String {
    let chars: Vec<char> = entry.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' {
            let mut n = 0usize;
            while i < chars.len() && chars[i] == '\\' {
                n += 1;
                i += 1;
            }
            if n % 2 == 1 && i < chars.len() && chars[i] == esc {
                for _ in 0..(n - 1) {
                    out.push('\\');
                }
                out.push_str(repl);
                i += 1;
            } else {
                for _ in 0..n {
                    out.push('\\');
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn escape(translation: &str, nowrap: bool) -> String {
    let mut translation = translation.replace('\\', "\\\\").replace('"', "\\\"");
    if translation.contains('\n') {
        let new_line = "\"\n\"";
        translation = translation.replace('\n', &format!("\\n{new_line}"));
        if translation.ends_with(new_line) {
            translation.truncate(translation.len() - new_line.len());
        }
        if nowrap {
            translation = format!("{new_line}{translation}");
        }
    }
    translation.replace('\t', "\\t")
}

fn plural_info(lang: &str) -> Option<(usize, &'static str)> {
    let lang = lang.to_ascii_lowercase();
    PLURALS
        .iter()
        .find(|(l, _, _)| *l == lang)
        .map(|(_, n, e)| (*n, *e))
}

static COMMENT_FUZZY: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#, fuzzy$").unwrap());
static COMMENT_FUZZY_OTHER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#,.* fuzzy.*$").unwrap());
static COMMENT_FUZZY_STRIP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(.*), fuzzy(.*)").unwrap());
static COMMENT_FUZZY_MSGID: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"#\|.* msgid.*"(.*)""#).unwrap());
static COMMENT_FUZZY_MSGCTX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"#\|.* msgctxt\s+"(.*)""#).unwrap());
static COMMENT_NOWRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#,.* no-wrap.*$").unwrap());
static COMMENT_TRANSLATOR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^# (.*)$").unwrap());
static COMMENT_EXTRACTED: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#\. (.*)$").unwrap());
static COMMENT_REFERENCE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#: (.*)$").unwrap());
static MSG_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^msgid(_plural)?\s+"(.*)""#).unwrap());
static MSG_STR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^msgstr(\[([0-9]+)])?\s+"(.*)""#).unwrap());
static MSG_CTX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^msgctxt\s+"(.*)""#).unwrap());
static MSG_OTHER: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^"(.*)""#).unwrap());
static MSG_FUZZY: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^#\|\s"(.*)""#).unwrap());
static PLURAL_FORMS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Plural-Forms: *nplurals= *([0-9]+) *; *plural").unwrap());

const PLURALS: &[(&str, usize, &str)] = &[
    ("ach", 2, "(n > 1)"),
    ("af", 2, "(n != 1)"),
    ("ak", 2, "(n > 1)"),
    ("am", 2, "(n > 1)"),
    ("an", 2, "(n != 1)"),
    (
        "ar",
        6,
        " n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5",
    ),
    ("arn", 2, "(n > 1)"),
    ("ast", 2, "(n != 1)"),
    ("ay", 1, "0"),
    ("az", 2, "(n != 1) "),
    (
        "be",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("bg", 2, "(n != 1)"),
    ("bn", 2, "(n != 1)"),
    ("bo", 1, "0"),
    ("br", 2, "(n > 1)"),
    ("brx", 2, "(n != 1)"),
    (
        "bs",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2) ",
    ),
    ("ca", 2, "(n != 1)"),
    ("cgg", 1, "0"),
    ("cs", 3, "(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2"),
    (
        "csb",
        3,
        "n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2",
    ),
    (
        "cy",
        4,
        " (n==1) ? 0 : (n==2) ? 1 : (n != 8 && n != 11) ? 2 : 3",
    ),
    ("da", 2, "(n != 1)"),
    ("de", 2, "(n != 1)"),
    ("doi", 2, "(n != 1)"),
    ("dz", 1, "0"),
    ("el", 2, "(n != 1)"),
    ("en", 2, "(n != 1)"),
    ("eo", 2, "(n != 1)"),
    ("es", 2, "(n != 1)"),
    ("et", 2, "(n != 1)"),
    ("eu", 2, "(n != 1)"),
    ("fa", 1, "0"),
    ("ff", 2, "(n != 1)"),
    ("fi", 2, "(n != 1)"),
    ("fil", 2, "n > 1"),
    ("fo", 2, "(n != 1)"),
    ("fr", 2, "(n > 1)"),
    ("fur", 2, "(n != 1)"),
    ("fy", 2, "(n != 1)"),
    ("ga", 5, "n==1 ? 0 : n==2 ? 1 : n<7 ? 2 : n<11 ? 3 : 4"),
    (
        "gd",
        4,
        "(n==1 || n==11) ? 0 : (n==2 || n==12) ? 1 : (n > 2 && n < 20) ? 2 : 3",
    ),
    ("gl", 2, "(n != 1)"),
    ("gu", 2, "(n != 1)"),
    ("gun", 2, "(n > 1)"),
    ("ha", 2, "(n != 1)"),
    ("he", 2, "(n != 1)"),
    ("hi", 2, "(n != 1)"),
    ("hne", 2, "(n != 1)"),
    ("hy", 2, "(n != 1)"),
    (
        "hr",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("hu", 2, "(n != 1)"),
    ("ia", 2, "(n != 1)"),
    ("id", 1, "0"),
    ("is", 2, "(n%10!=1 || n%100==11)"),
    ("it", 2, "(n != 1)"),
    ("ja", 1, "0"),
    ("jbo", 1, "0"),
    ("jv", 2, "n!=0"),
    ("ka", 1, "0"),
    ("kk", 1, "0"),
    ("km", 1, "0"),
    ("kn", 2, "(n!=1)"),
    ("ko", 1, "0"),
    ("ku", 2, "(n!= 1)"),
    ("kw", 4, " (n==1) ? 0 : (n==2) ? 1 : (n == 3) ? 2 : 3"),
    ("ky", 1, "0"),
    ("lb", 2, "(n != 1)"),
    ("ln", 2, "n>1"),
    ("lo", 1, "0"),
    (
        "lt",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && (n%100<10 or n%100>=20) ? 1 : 2)",
    ),
    ("lv", 3, "(n%10==1 && n%100!=11 ? 0 : n != 0 ? 1 : 2)"),
    ("mai", 2, "(n != 1)"),
    ("mfe", 2, "(n > 1)"),
    ("mg", 2, "(n > 1)"),
    ("mi", 2, "(n > 1)"),
    ("mk", 2, " n==1 || n%10==1 ? 0 : 1"),
    ("ml", 2, "(n != 1)"),
    ("mn", 2, "(n != 1)"),
    ("mni", 2, "(n != 1)"),
    ("mnk", 3, "(n==0 ? 0 : n==1 ? 1 : 2"),
    ("mr", 2, "(n != 1)"),
    ("ms", 1, "0"),
    (
        "mt",
        4,
        "(n==1 ? 0 : n==0 || ( n%100>1 && n%100<11) ? 1 : (n%100>10 && n%100<20 ) ? 2 : 3)",
    ),
    ("my", 1, "0"),
    ("nah", 2, "(n != 1)"),
    ("nap", 2, "(n != 1)"),
    ("nb", 2, "(n != 1)"),
    ("ne", 2, "(n != 1)"),
    ("nl", 2, "(n != 1)"),
    ("se", 2, "(n != 1)"),
    ("nn", 2, "(n != 1)"),
    ("no", 2, "(n != 1)"),
    ("nso", 2, "(n != 1)"),
    ("oc", 2, "(n > 1)"),
    ("or", 2, "(n != 1)"),
    ("ps", 2, "(n != 1)"),
    ("pa", 2, "(n != 1)"),
    ("pap", 2, "(n != 1)"),
    (
        "pl",
        3,
        "(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("pms", 2, "(n != 1)"),
    ("pt", 2, "(n != 1)"),
    ("rm", 2, "(n!=1)"),
    (
        "ro",
        3,
        "(n==1 ? 0 : (n==0 || (n%100 > 0 && n%100 < 20)) ? 1 : 2)",
    ),
    (
        "ru",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("rw", 2, "(n != 1)"),
    ("sah", 1, "0"),
    ("sat", 2, "(n != 1)"),
    ("sco", 2, "(n != 1)"),
    ("sd", 2, "(n != 1)"),
    ("si", 2, "(n != 1)"),
    ("sk", 3, "(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2"),
    (
        "sl",
        4,
        "(n%100==1 ? 1 : n%100==2 ? 2 : n%100==3 || n%100==4 ? 3 : 0)",
    ),
    ("so", 2, "n != 1"),
    ("son", 2, "(n != 1)"),
    ("sq", 2, "(n != 1)"),
    (
        "sr",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("su", 1, "0"),
    ("sw", 2, "(n != 1)"),
    ("sv", 2, "(n != 1)"),
    ("ta", 2, "(n != 1)"),
    ("te", 2, "(n != 1)"),
    ("tg", 2, "(n > 1)"),
    ("ti", 2, "n > 1"),
    ("th", 1, "0"),
    ("tk", 2, "(n != 1)"),
    ("tr", 2, "(n>1)"),
    ("tt", 1, "0"),
    ("ug", 1, "0"),
    (
        "uk",
        3,
        "(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)",
    ),
    ("ur", 2, "(n != 1)"),
    ("uz", 2, "(n > 1)"),
    ("vi", 1, "0"),
    ("wa", 2, "(n > 1)"),
    ("wo", 1, "0"),
    ("yo", 2, "(n != 1)"),
    ("zh", 1, "0 "),
];
