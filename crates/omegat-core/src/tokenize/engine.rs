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
    if ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}' || ch == '_' {
        return true;
    }
    // UAX#29 Extend: virama, nuktas, Thai/Arabic combining marks stay in the word
    // (Lucene StandardTokenizer / WordBreak).
    let u = ch as u32;
    (0x0300..=0x036F).contains(&u)
        || (0x064B..=0x065F).contains(&u)
        || (0x0670..=0x0670).contains(&u)
        || (0x06D6..=0x06ED).contains(&u)
        || (0x0900..=0x0903).contains(&u)
        || (0x093A..=0x094F).contains(&u)
        || (0x0951..=0x0957).contains(&u)
        || (0x0962..=0x0963).contains(&u)
        || (0x0E31..=0x0E31).contains(&u)
        || (0x0E34..=0x0E3A).contains(&u)
        || (0x0E47..=0x0E4E).contains(&u)
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

/// Java `BaseTokenizer.tokenizeWords`: analyzer terms only (no surface pair).
#[allow(dead_code)]
pub fn lucene_word_tokens(
    text: &str,
    mode: StemmingMode,
    stem: impl Fn(&str, bool) -> String,
    stopwords: &[&str],
) -> Vec<String> {
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
        out.push(term);
    }
    out
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
    stopwords.iter().any(|s| {
        s.eq_ignore_ascii_case(word) || *s == word || fold_lower(s) == fold_lower(word)
    })
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

/// Java `WordIterator` + `BreakIterator.getWordInstance` heuristics used by
/// `DefaultTokenizer` / `Statistics.numberOfWords`.
///
/// CJK stays a script run (not per-ideograph). Latin hyphen joins only
/// letter–letter (`Content-Type`, `X-Language`), not letter–digit (`UTF-8`),
/// matching UAX #29 WB6 (`AHLetter × MidNumLet × AHLetter`).
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
            // Java `BreakIterator.getWordInstance` keeps a line break (`\n`)
            // separate from a following space run (`        `).
            let mut j = i + 1;
            if matches!(ch, '\n' | '\r') {
                if ch == '\r' && j < chars.len() && chars[j].1 == '\n' {
                    j += 1;
                }
            } else {
                while j < chars.len() && chars[j].1.is_whitespace() && !matches!(chars[j].1, '\n' | '\r')
                {
                    j += 1;
                }
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
            while j < chars.len() {
                let next = chars[j].1;
                if is_word_char(next) && (script_kind(next) == script || is_intra_word_mark(next)) {
                    j += 1;
                    continue;
                }
                // WB6 MidNumLet: letter -/. letter stays one token (`blog.discourse.org`,
                // `understanding-discourse-trust-levels`). `UTF-8` stays two.
                if matches!(next, '-' | '.')
                    && j + 1 < chars.len()
                    && chars[j - 1].1.is_alphabetic()
                    && chars[j + 1].1.is_alphabetic()
                    && script_kind(chars[j + 1].1) == script
                {
                    j += 1;
                    continue;
                }
                // Java BreakIterator: `upload_bucket` stays one token; `s3_upload`
                // splits as `s3` + `_` + `upload` (underscore after a digit).
                if next == '_'
                    && j + 1 < chars.len()
                    && chars[j - 1].1.is_alphabetic()
                    && chars[j + 1].1.is_alphabetic()
                    && script_kind(chars[j + 1].1) == script
                {
                    j += 1;
                    continue;
                }
                break;
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
    // ASCII apostrophe only: Java `BreakIterator` keeps `can't` together but
    // splits U+2019 (`don’t` → `don` + `t`).
    ch.is_alphanumeric() || ch == '\''
}

fn is_intra_word_mark(ch: char) -> bool {
    ch == '\''
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
    } else if ch.is_ascii_alphanumeric() || ch.is_alphabetic() || ch == '_' {
        ScriptKind::Latin
    } else {
        ScriptKind::Other
    }
}

pub fn is_omegat_tag(s: &str) -> bool {
    static RE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"^</?[a-zA-Z]+[0-9]+/?>$").unwrap());
    RE.is_match(s)
}

fn find_tag_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if !rest.starts_with('<') {
        return None;
    }
    let close = rest.find('>')?;
    let tag = &rest[..=close];
    // Java `WordIterator` groups only `OMEGAT_TAG_ONLY` (`x0`, `/x0`). HTML
    // `<a href="...">` must stay split so `tokenizeVerbatim` / word counts match.
    if is_omegat_tag(tag) {
        return Some(start + close + 1);
    }
    None
}

#[cfg(test)]
mod word_iterator_tests {
    use super::*;

    fn texts(s: &str) -> Vec<String> {
        word_iterator_surfaces(s)
            .into_iter()
            .map(|t| t.text.to_string())
            .collect()
    }

    #[test]
    fn java_verbatim_contraction_and_linebreak() {
        assert_eq!(
            texts("can't have emoji"),
            ["can't", " ", "have", " ", "emoji"]
        );
        let tm = "sorry, this account confirmation link is no longer valid. perhaps your account is\n        already";
        let got = texts(tm);
        assert_eq!(got[got.len() - 3], "\n", "{got:?}");
        assert_eq!(got[got.len() - 2], "        ", "{got:?}");
        assert_eq!(got[got.len() - 1], "already", "{got:?}");
        assert_eq!(
            texts("s3_upload_bucket"),
            ["s3", "_", "upload_bucket"]
        );
        let flag = "This badge is granted the first time you flag a post. Flagging is how we all help keep this a nice place for everyone. If you notice any posts that require moderator attention for any reason please don’t hesitate to flag. If you see a problem, :flag_black: flag it!\n";
        let words: Vec<_> = texts(flag)
            .into_iter()
            .filter(|t| t.chars().any(|c| c.is_alphanumeric()))
            .collect();
        assert_eq!(words.len(), 50, "{words:?}");
        assert!(words.contains(&"don".into()), "{words:?}");
        assert!(words.contains(&"t".into()), "{words:?}");
        assert_eq!(texts("blog.discourse.org"), ["blog.discourse.org"]);
        let href = "<a href=\"https://blog.discourse.org/2018/06/understanding-discourse-trust-levels/\">Granted</a> invitations, group messaging, more likes";
        let href_words: Vec<_> = texts(href)
            .into_iter()
            .filter(|t| t.chars().any(|c| c.is_alphanumeric()))
            .collect();
        assert_eq!(href_words.len(), 14, "{href_words:?}");
    }
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
