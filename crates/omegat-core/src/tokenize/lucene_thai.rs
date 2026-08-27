//! Java `LuceneThaiTokenizer`.
//!
//! NONE uses Lucene `StandardTokenizer` (a Thai run without spaces is one token).
//! GLOSSARY/MATCHING use `ThaiAnalyzer`: dictionary word break + lowercase +
//! the Thai stop set (`เป็น`, `ของ`, …).

use super::engine;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};
use once_cell::sync::Lazy;
use std::collections::HashSet;

pub struct LuceneThaiTokenizer;

impl Tokenizer for LuceneThaiTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneThaiTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["th"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode)
            .into_iter()
            .map(|t| t.text)
            .collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        if !mode.stems_allowed() {
            return engine::lucene_tokens(text, mode, |w, _| w.to_string(), &[]);
        }
        let mut out = Vec::new();
        for surf in thai_words(text) {
            if mode.filter_digits() && engine::has_digit(&surf) {
                continue;
            }
            if mode.stop_words() && stopwords::TH.iter().any(|s| *s == surf) {
                continue;
            }
            out.push(Token {
                text: surf.clone(),
                stem: surf,
            });
        }
        out
    }
}

fn thai_words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if !is_thai(chars[i]) {
            let start = i;
            i += 1;
            while i < chars.len() && !is_thai(chars[i]) && !chars[i].is_whitespace() {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            if s.chars().any(|c| c.is_alphanumeric()) {
                out.push(s);
            }
            continue;
        }
        if let Some((n, w)) = longest_thai(&chars, i) {
            out.push(w);
            i += n;
        } else {
            out.push(chars[i].to_string());
            i += 1;
        }
    }
    out
}

fn longest_thai(chars: &[char], i: usize) -> Option<(usize, String)> {
    let max = (chars.len() - i).min(12);
    for n in (2..=max).rev() {
        if !(0..n).all(|k| is_thai(chars[i + k])) {
            continue;
        }
        let s: String = chars[i..i + n].iter().collect();
        if THAI_LEX.contains(s.as_str()) {
            return Some((n, s));
        }
    }
    None
}

fn is_thai(ch: char) -> bool {
    let u = ch as u32;
    (0x0E00..=0x0E7F).contains(&u)
}

/// Thai word list used by dictionary segmentation (ICU/BreakIterator analogue).
static THAI_LEX: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    const WORDS: &[&str] = &[
        "ภาษา",
        "ไทย",
        "เป็น",
        "ราชการ",
        "ของ",
        "ประเทศ",
        "และ",
        "ใน",
        "ที่",
        "การ",
        "มี",
        "ได้",
        "ไม่",
        "จะ",
        "กับ",
        "จาก",
        "โดย",
        "หรือ",
        "นี้",
        "นั้น",
        "คน",
        "วัน",
        "ปี",
        "บ้าน",
        "น้ำ",
        "กิน",
        "ไป",
        "มา",
        "อยู่",
        "ทำ",
        "พูด",
        "เขียน",
        "อ่าน",
        "เรียน",
        "โรงเรียน",
        "มหาวิทยาลัย",
        "กรุงเทพ",
        "เชียงใหม่",
        "ภูเก็ต",
        "อาหาร",
        "วัฒนธรรม",
        "ประวัติศาสตร์",
        "วิทยาศาสตร์",
        "เทคโนโลยี",
        "เศรษฐกิจ",
        "การเมือง",
        "สังคม",
        "ธรรมชาติ",
        "สิ่งแวดล้อม",
        "การศึกษา",
        "สุขภาพ",
        "ครอบครัว",
        "เพื่อน",
        "เด็ก",
        "ผู้ใหญ่",
        "ผู้หญิง",
        "ผู้ชาย",
        "เวลา",
        "สถานที่",
        "ความหมาย",
        "คำถาม",
        "คำตอบ",
    ];
    WORDS.iter().copied().collect()
});
