//! Shared Lucene-like tokenization used by every `Lucene*Tokenizer` module.
//!
//! `NONE` is Lucene `StandardTokenizer` (Unicode words, punctuation dropped).
//! Stemming modes emit the analyzer term and, when it differs, the original
//! surface form — the same pairing as `BaseTokenizer.tokenizeToStrings`.

use super::{StemmingMode, Token};

#[derive(Clone, Copy)]
pub struct Surface<'a> {
    pub text: &'a str,
    #[allow(dead_code)]
    pub start: usize,
    #[allow(dead_code)]
    pub end: usize,
}

pub fn standard_surfaces(text: &str) -> Vec<Surface<'_>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, ch) in text.char_indices() {
        if is_token_char(ch) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push(Surface {
                text: &text[s..i],
                start: s,
                end: i,
            });
        }
    }
    if let Some(s) = start {
        out.push(Surface {
            text: &text[s..],
            start: s,
            end: text.len(),
        });
    }
    out
}

fn is_token_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}' || ch == '_'
}

pub fn has_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit() || c.is_numeric())
}

pub fn accept_token(token: &str, filter_digits: bool) -> bool {
    if token.is_empty() || token.chars().all(char::is_whitespace) {
        return false;
    }
    if filter_digits && has_digit(token) {
        return false;
    }
    true
}

pub fn lucene_words_to_strings(
    text: &str,
    mode: StemmingMode,
    stem: impl Fn(&str, bool) -> String,
    stopwords: &[&str],
) -> Vec<String> {
    lucene_tokens(text, mode, stem, stopwords)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

pub fn lucene_tokens(
    text: &str,
    mode: StemmingMode,
    stem: impl Fn(&str, bool) -> String,
    stopwords: &[&str],
) -> Vec<Token> {
    let stems_allowed = mode.stems_allowed();
    let stop = mode.stop_words();
    let filter_digits = mode.filter_digits();
    let full = mode.full();
    let mut out = Vec::new();
    for surf in standard_surfaces(text) {
        if !accept_token(surf.text, filter_digits) {
            continue;
        }
        let lower = fold_lower(surf.text);
        if stop && is_stop(&lower, stopwords) {
            continue;
        }
        let term = if stems_allowed {
            stem(&lower, full)
        } else {
            surf.text.to_string()
        };
        if !accept_token(&term, filter_digits) {
            continue;
        }
        if stop && is_stop(&term, stopwords) {
            continue;
        }
        out.push(Token {
            text: term.clone(),
            stem: term.clone(),
        });
        if stems_allowed && fold_lower(surf.text) != fold_lower(&term) {
            out.push(Token {
                text: surf.text.to_string(),
                stem: term,
            });
        }
    }
    out
}

fn is_stop(word: &str, stopwords: &[&str]) -> bool {
    stopwords.iter().any(|s| s.eq_ignore_ascii_case(word))
}

pub fn fold_lower(s: &str) -> String {
    s.replace('İ', "i").to_lowercase().replace("i\u{307}", "i")
}

/// DefaultTokenizer / WordIterator: letter-bearing tokens, OmegaT tags skipped.
pub fn default_words(text: &str) -> Vec<String> {
    default_word_tokens(text).into_iter().map(|t| t.text).collect()
}

pub fn default_word_tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for surf in word_iterator_surfaces(text) {
        if is_omegat_tag(surf.text) {
            continue;
        }
        if !surf.text.chars().any(|c| c.is_alphabetic()) {
            continue;
        }
        out.push(Token {
            text: surf.text.to_string(),
            stem: surf.text.to_string(),
        });
    }
    out
}

pub fn word_iterator_surfaces(text: &str) -> Vec<Surface<'_>> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (start, ch) = chars[i];
        if ch == '<' {
            if let Some(end) = find_tag_end(text, start) {
                out.push(Surface {
                    text: &text[start..end],
                    start,
                    end,
                });
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
            out.push(Surface {
                text: &text[start..end],
                start,
                end,
            });
            i = j;
            continue;
        }
        if is_word_char(ch) {
            let script = script_kind(ch);
            let mut j = i + 1;
            while j < chars.len() && is_word_char(chars[j].1) && script_kind(chars[j].1) == script {
                j += 1;
            }
            let end = if j < chars.len() { chars[j].0 } else { text.len() };
            out.push(Surface {
                text: &text[start..end],
                start,
                end,
            });
            i = j;
            continue;
        }
        let end = if i + 1 < chars.len() { chars[i + 1].0 } else { text.len() };
        out.push(Surface {
            text: &text[start..end],
            start,
            end,
        });
        i += 1;
    }
    out
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}' || ch == '-'
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScriptKind {
    Latin,
    Cjk,
    Kana,
    Other,
}

fn script_kind(ch: char) -> ScriptKind {
    let u = ch as u32;
    if (0x3040..=0x30FF).contains(&u) || (0x31F0..=0x31FF).contains(&u) {
        ScriptKind::Kana
    } else if (0x3400..=0x9FFF).contains(&u) || (0xF900..=0xFAFF).contains(&u) {
        ScriptKind::Cjk
    } else if ch.is_ascii_alphanumeric() || ch.is_alphabetic() {
        ScriptKind::Latin
    } else {
        ScriptKind::Other
    }
}

pub fn is_omegat_tag(s: &str) -> bool {
    let b = s.as_bytes();
    b.first() == Some(&b'<') && b.last() == Some(&b'>') && s.len() >= 3
}

fn find_tag_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if !rest.starts_with('<') {
        return None;
    }
    let close = rest.find('>')?;
    let tag = &rest[..=close];
    if tag.len() >= 3 && tag.as_bytes()[1].is_ascii_alphabetic() || tag.starts_with("</") || tag.starts_with("<x") {
        return Some(start + close + 1);
    }
    None
}

/// Lucene CJKTokenizer: overlapping CJK bigrams, Latin words intact.
pub fn cjk_bigrams(text: &str, lowercase: bool) -> Vec<Token> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut cjk: Vec<char> = Vec::new();
    let flush_latin = |buf: &mut String, out: &mut Vec<Token>, lowercase: bool| {
        if !buf.is_empty() {
            let w = if lowercase { buf.to_lowercase() } else { buf.clone() };
            out.push(Token {
                stem: w.clone(),
                text: w,
            });
            buf.clear();
        }
    };
    let flush_cjk = |cjk: &mut Vec<char>, out: &mut Vec<Token>| {
        if cjk.len() == 1 {
            let s = cjk[0].to_string();
            out.push(Token {
                stem: s.clone(),
                text: s,
            });
        } else if cjk.len() >= 2 {
            for w in cjk.windows(2) {
                let s: String = w.iter().collect();
                out.push(Token {
                    stem: s.clone(),
                    text: s,
                });
            }
        }
        cjk.clear();
    };
    for ch in text.chars() {
        if ch.is_whitespace() || is_cjk_punct(ch) {
            flush_latin(&mut buf, &mut out, lowercase);
            flush_cjk(&mut cjk, &mut out);
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            flush_cjk(&mut cjk, &mut out);
            buf.push(ch);
            continue;
        }
        if is_cjk(ch) {
            flush_latin(&mut buf, &mut out, lowercase);
            cjk.push(ch);
            continue;
        }
        flush_latin(&mut buf, &mut out, lowercase);
        flush_cjk(&mut cjk, &mut out);
    }
    flush_latin(&mut buf, &mut out, lowercase);
    flush_cjk(&mut cjk, &mut out);
    out
}

fn is_cjk(ch: char) -> bool {
    let u = ch as u32;
    (0x3040..=0x30FF).contains(&u)
        || (0x3400..=0x9FFF).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xAC00..=0xD7AF).contains(&u)
}

fn is_cjk_punct(ch: char) -> bool {
    ch.is_ascii_punctuation() || matches!(ch, '。' | '、' | '「' | '」' | '（' | '）' | '！' | '？' | '：' | '；')
}
