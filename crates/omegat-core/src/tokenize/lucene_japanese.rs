//! Java `LuceneJapaneseTokenizer`.
//!
//! Tag joining / blanking follows the Java class. Word breaking is script-run
//! plus an OmegaT-tag scanner (Kuromoji is not embedded).
use super::engine;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneJapaneseTokenizer;

impl Tokenizer for LuceneJapaneseTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneJapaneseTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ja"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode).into_iter().map(|t| t.text).collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        if mode.stems_allowed() {
            let blanked = blank_out_tags(text);
            let mut out = Vec::new();
            for surf in ja_surfaces(&blanked) {
                if engine::is_omegat_tag(surf) {
                    continue;
                }
                if mode.filter_digits() && engine::has_digit(surf) {
                    continue;
                }
                if surf.chars().all(|c| c.is_whitespace() || is_ja_punct(c)) {
                    continue;
                }
                if surf.chars().all(|c| !c.is_alphanumeric() && !is_cjk(c)) {
                    continue;
                }
                out.push(Token {
                    text: surf.to_string(),
                    stem: surf.to_string(),
                });
            }
            out
        } else {
            ja_surfaces(text)
                .into_iter()
                .filter(|s| {
                    if s.chars().all(char::is_whitespace) {
                        return false;
                    }
                    if mode.filter_digits() && engine::has_digit(s) {
                        return false;
                    }
                    true
                })
                .map(|s| Token {
                    text: s.to_string(),
                    stem: s.to_string(),
                })
                .collect()
        }
    }
}

fn blank_out_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let b = text.as_bytes();
    while i < text.len() {
        if b[i] == b'<' {
            if let Some(end) = next_omegat_tag_end(text, i) {
                out.push_str(&" ".repeat(end - i));
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn next_omegat_tag_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if !rest.starts_with('<') {
        return None;
    }
    let close = rest.find('>')?;
    let tag = &rest[..=close];
    if is_omegat_style_tag(tag) {
        Some(start + close + 1)
    } else {
        None
    }
}

fn is_omegat_style_tag(tag: &str) -> bool {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim_end_matches('/');
    let inner = inner.trim_start_matches('/');
    if inner.is_empty() {
        return false;
    }
    let mut chars = inner.chars();
    let first = chars.next().unwrap();
    first.is_ascii_alphabetic() && inner.chars().all(|c| c.is_ascii_alphanumeric())
}

fn ja_surfaces(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (start, ch) = chars[i];
        if ch == '<' {
            if let Some(end) = next_omegat_tag_end(text, start) {
                out.push(&text[start..end]);
                i = chars.iter().position(|(o, _)| *o >= end).unwrap_or(chars.len());
                continue;
            }
        }
        if ch.is_whitespace() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1.is_whitespace() {
                j += 1;
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            out.push(&text[start..end]);
            i = j;
            continue;
        }
        if is_cjk(ch) || ch.is_alphanumeric() {
            let kind = ja_kind(ch);
            let mut j = i + 1;
            while j < chars.len() && ja_kind(chars[j].1) == kind && chars[j].1 != '<' {
                j += 1;
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            out.push(&text[start..end]);
            i = j;
            continue;
        }
        let end = if i + 1 < chars.len() { chars[i + 1].0 } else { text.len() };
        out.push(&text[start..end]);
        i += 1;
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JaKind {
    Kanji,
    Hira,
    Kata,
    Latin,
    Digit,
    Other,
}

fn ja_kind(ch: char) -> JaKind {
    let u = ch as u32;
    if ch.is_ascii_digit() {
        JaKind::Digit
    } else if ch.is_ascii_alphabetic() {
        JaKind::Latin
    } else if (0x3040..=0x309F).contains(&u) {
        JaKind::Hira
    } else if (0x30A0..=0x30FF).contains(&u) {
        JaKind::Kata
    } else if is_cjk(ch) {
        JaKind::Kanji
    } else {
        JaKind::Other
    }
}

fn is_cjk(ch: char) -> bool {
    let u = ch as u32;
    (0x3040..=0x30FF).contains(&u) || (0x3400..=0x9FFF).contains(&u) || (0xF900..=0xFAFF).contains(&u)
}

fn is_ja_punct(ch: char) -> bool {
    matches!(ch, '。' | '、' | '「' | '」' | '（' | '）' | '！' | '？' | '：' | '；' | '.' | ',' | '<' | '>')
}
