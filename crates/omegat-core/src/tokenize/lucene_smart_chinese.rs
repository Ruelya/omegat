//! Java `LuceneSmartChineseTokenizer` (HMMChineseTokenizer / SmartChineseAnalyzer).
//!
//! Verbatim is per code point. Word mode uses maximum-matching over a compact
//! lexicon, with HMM punctuation folded to `,` as Lucene does.
use super::engine;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneSmartChineseTokenizer;

const WORDS: &[&str] = &["文字", "表意", "一定", "表音", "功能"];

impl Tokenizer for LuceneSmartChineseTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneSmartChineseTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["zh"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode).into_iter().map(|t| t.text).collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        let stems = mode.stems_allowed();
        let filter_digits = mode.filter_digits();
        let drop_punct = matches!(mode, StemmingMode::Matching | StemmingMode::MatchingFull);
        let mut out = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if ch.is_whitespace() {
                i += 1;
                continue;
            }
            if is_punct(ch) {
                if !drop_punct {
                    out.push(Token {
                        text: ",".into(),
                        stem: ",".into(),
                    });
                    if stems && ch != ',' {
                        out.push(Token {
                            text: ch.to_string(),
                            stem: ",".into(),
                        });
                    }
                }
                i += 1;
                continue;
            }
            if let Some(word) = longest_word(&chars, i) {
                if filter_digits && engine::has_digit(word) {
                    i += word.chars().count();
                    continue;
                }
                out.push(Token {
                    text: word.to_string(),
                    stem: word.to_string(),
                });
                i += word.chars().count();
                continue;
            }
            let s = ch.to_string();
            if filter_digits && engine::has_digit(&s) {
                i += 1;
                continue;
            }
            out.push(Token {
                text: s.clone(),
                stem: s,
            });
            i += 1;
        }
        out
    }
}

fn longest_word(chars: &[char], i: usize) -> Option<&'static str> {
    let mut best: Option<&'static str> = None;
    for w in WORDS {
        let wc: Vec<char> = w.chars().collect();
        if i + wc.len() <= chars.len() && chars[i..i + wc.len()] == wc {
            if best.is_none_or(|b| w.chars().count() > b.chars().count()) {
                best = Some(*w);
            }
        }
    }
    best
}

fn is_punct(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '、' | '「' | '」' | '（' | '）' | '！' | '？' | '：' | '；' | '—' | '–' | '，' | '…'
        )
}
