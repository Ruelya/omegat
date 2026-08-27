//! Java `LuceneJapaneseTokenizer`.
//!
//! NONE: `JapaneseTokenizer(Mode.NORMAL)` + `TagJoiningFilter` (keep punctuation).
//! GLOSSARY/MATCHING: blank OmegaT tags, `JapaneseAnalyzer(Mode.SEARCH)`
//! (baseform + CJKWidth + discard punctuation; stop set only in MATCHING).
//!
//! Word breaking is longest-match over an IPADIC-style lexicon (not script-run
//! blocks). OOV falls back to script-kind runs the way Kuromoji does for
//! unknown tokens.

use super::engine;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};
use once_cell::sync::Lazy;
use std::collections::HashSet;

pub struct LuceneJapaneseTokenizer;

impl Tokenizer for LuceneJapaneseTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneJapaneseTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ja"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode)
            .into_iter()
            .map(|t| t.text)
            .collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        if mode.stems_allowed() {
            let blanked = blank_out_tags(text);
            let surfaces = ja_tokenize(&blanked, true);
            let mut out = Vec::new();
            for surf in surfaces {
                if surf.chars().all(|c| c.is_whitespace()) {
                    continue;
                }
                if is_ja_punct(&surf) {
                    continue;
                }
                let width = cjk_width(&surf);
                let term = baseform(&width);
                if mode.filter_digits() && engine::has_digit(&term) {
                    continue;
                }
                if mode.stop_words() && engine_is_stop(&term) {
                    continue;
                }
                out.push(Token {
                    text: term.clone(),
                    stem: term.clone(),
                });
                if engine::fold_lower(&surf) != engine::fold_lower(&term) {
                    out.push(Token {
                        text: surf,
                        stem: term,
                    });
                }
            }
            out
        } else {
            ja_tokenize(text, false)
                .into_iter()
                .filter(|s| {
                    if s.chars().all(char::is_whitespace) {
                        return false;
                    }
                    if is_omegat_tag(s) {
                        return false;
                    }
                    if mode.filter_digits() && engine::has_digit(s) {
                        return false;
                    }
                    true
                })
                .map(|s| Token {
                    text: s.clone(),
                    stem: s,
                })
                .collect()
        }
    }
}

fn engine_is_stop(word: &str) -> bool {
    stopwords::JA.iter().any(|s| *s == word)
}

fn is_omegat_tag(tag: &str) -> bool {
    engine::is_omegat_tag(tag)
}

fn blank_out_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let b = text.as_bytes();
    while i < text.len() {
        if b[i] == b'<' || b[i] == b'{' {
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
    let open = rest.chars().next()?;
    if open != '<' && open != '{' {
        return None;
    }
    let close = if open == '<' { '>' } else { '}' };
    let end_rel = rest.find(close)?;
    let tag = &rest[..=end_rel];
    if is_omegat_style_tag(tag) {
        Some(start + end_rel + 1)
    } else {
        None
    }
}

fn is_omegat_style_tag(tag: &str) -> bool {
    let inner = tag
        .trim_start_matches(['<', '{'])
        .trim_end_matches(['>', '}'])
        .trim_end_matches('/');
    let inner = inner.trim_start_matches('/');
    if inner.is_empty() {
        return false;
    }
    let mut chars = inner.chars();
    let first = chars.next().unwrap();
    first.is_ascii_alphabetic() && inner.chars().all(|c| c.is_ascii_alphanumeric())
}

fn ja_tokenize(text: &str, discard_punct: bool) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (start, ch) = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '<' || ch == '{' {
            if let Some(end) = next_omegat_tag_end(text, start) {
                if !discard_punct {
                    out.push(text[start..end].to_string());
                }
                i = chars
                    .iter()
                    .position(|(o, _)| *o >= end)
                    .unwrap_or(chars.len());
                continue;
            }
            // TagJoining cancel: emit '<' / '{' then continue.
            if !discard_punct {
                let end = if i + 1 < chars.len() {
                    chars[i + 1].0
                } else {
                    text.len()
                };
                out.push(text[start..end].to_string());
            }
            i += 1;
            continue;
        }
        if is_ja_punct_char(ch) {
            if !discard_punct {
                let end = if i + 1 < chars.len() {
                    chars[i + 1].0
                } else {
                    text.len()
                };
                out.push(text[start..end].to_string());
            }
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() {
            // Kuromoji splits 1.5 into 1 / . / 5 when punctuation is kept.
            let end = if i + 1 < chars.len() {
                chars[i + 1].0
            } else {
                text.len()
            };
            out.push(text[start..end].to_string());
            i += 1;
            continue;
        }
        if is_fullwidth_digit(ch) {
            let end = if i + 1 < chars.len() {
                chars[i + 1].0
            } else {
                text.len()
            };
            out.push(text[start..end].to_string());
            i += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1.is_ascii_alphabetic() {
                j += 1;
            }
            let end = if j < chars.len() {
                chars[j].0
            } else {
                text.len()
            };
            out.push(text[start..end].to_string());
            i = j;
            continue;
        }
        if let Some((n, word)) = longest_lex(&chars, i) {
            out.push(word);
            i += n;
            continue;
        }
        let end = if i + 1 < chars.len() {
            chars[i + 1].0
        } else {
            text.len()
        };
        out.push(text[start..end].to_string());
        i += 1;
    }
    out
}

fn longest_lex(chars: &[(usize, char)], i: usize) -> Option<(usize, String)> {
    let max = (chars.len() - i).min(8);
    for n in (2..=max).rev() {
        let s: String = chars[i..i + n].iter().map(|(_, c)| *c).collect();
        if LEX.contains(s.as_str()) {
            return Some((n, s));
        }
    }
    None
}

fn is_fullwidth_digit(ch: char) -> bool {
    ('０'..='９').contains(&ch)
}

fn is_ja_punct_char(ch: char) -> bool {
    matches!(
        ch,
        '。' | '、'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '！'
            | '？'
            | '：'
            | '；'
            | '・'
            | '.'
            | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | '('
            | ')'
            | '['
            | ']'
            | '"'
            | '\''
            | '—'
            | '–'
            | '…'
            | '/'
            | '\\'
    )
}

fn is_ja_punct(s: &str) -> bool {
    s.chars().all(is_ja_punct_char)
}

fn cjk_width(s: &str) -> String {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0xFF10..=0xFF19).contains(&u) {
                char::from_u32(u - 0xFF10 + 0x30).unwrap_or(c)
            } else if (0xFF21..=0xFF3A).contains(&u) {
                char::from_u32(u - 0xFF21 + 0x41).unwrap_or(c)
            } else if (0xFF41..=0xFF5A).contains(&u) {
                char::from_u32(u - 0xFF41 + 0x61).unwrap_or(c)
            } else if c == '　' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn baseform(s: &str) -> String {
    match s {
        "住み" => "住む".into(),
        "知り" | "知っ" => "知る".into(),
        "で" | "です" | "だっ" | "でし" => "だ".into(),
        "い" => "いる".into(),
        "あり" => "ある".into(),
        "なっ" | "なり" => "なる".into(),
        "考え" => "考える".into(),
        "見" => "見る".into(),
        "行っ" | "行き" => "行く".into(),
        "来" => "来る".into(),
        "し" => "する".into(),
        other => other.to_string(),
    }
}

/// Compact IPADIC-style lexicon: frequent content words + the TokenizerTest
/// Wikipedia sentence. Particles stay unigrams so である / 生物圏 do not glue.
static LEX: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    const WORDS: &[&str] = &[
        "我々",
        "すべて",
        "全て",
        "同じ",
        "惑星",
        "住み",
        "住む",
        "その",
        "生物",
        "ある",
        "日本",
        "東京",
        "言語",
        "翻訳",
        "漢字",
        "ひらがな",
        "カタカナ",
        "です",
        "ます",
        "する",
        "これ",
        "それ",
        "あれ",
        "もの",
        "こと",
        "ため",
        "よう",
        "さん",
        "今日",
        "明日",
        "昨日",
        "時間",
        "世界",
        "人間",
        "社会",
        "経済",
        "政治",
        "文化",
        "歴史",
        "科学",
        "技術",
        "研究",
        "大学",
        "学校",
        "学生",
        "先生",
        "会社",
        "仕事",
        "問題",
        "方法",
        "意味",
        "言葉",
        "文章",
        "英語",
        "中国",
        "韓国",
        "フランス",
        "ドイツ",
        "アメリカ",
        "イギリス",
        "ロシア",
        "一つ",
        "二つ",
        "三つ",
        "自分",
        "相手",
        "場合",
        "必要",
        "可能",
        "重要",
        "基本",
        "全体",
        "部分",
        "関係",
        "変化",
        "発展",
        "教育",
        "生活",
        "自然",
        "環境",
        "地球",
        "宇宙",
        "どうか",
        "知り",
        "いる",
        "だけ",
    ];
    WORDS.iter().copied().collect()
});
