//! Language-aware tokenization. CJK uses overlapping character bigrams
//! (Lucene CJKTokenizer). Latin uses Unicode words + a compact stemmer.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub stem: String,
}

pub fn tokenize(text: &str, lang: &str) -> Vec<Token> {
    let lang = lang_base(lang);
    if matches!(lang, "zh" | "ja" | "th" | "km") {
        return cjk_bigrams(text);
    }
    text.to_lowercase()
        .unicode_words()
        .map(|w| Token {
            stem: stem(w, lang),
            text: w.to_string(),
        })
        .collect()
}

/// Lucene CJKTokenizer: overlapping character bigrams, Latin words kept intact.
fn cjk_bigrams(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut cjk: Vec<char> = Vec::new();
    let flush_latin = |buf: &mut String, out: &mut Vec<Token>| {
        if !buf.is_empty() {
            let w = buf.to_lowercase();
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
        if ch.is_whitespace() || ch.is_ascii_punctuation() {
            flush_latin(&mut buf, &mut out);
            flush_cjk(&mut cjk, &mut out);
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            flush_cjk(&mut cjk, &mut out);
            buf.push(ch);
            continue;
        }
        flush_latin(&mut buf, &mut out);
        cjk.push(ch);
    }
    flush_latin(&mut buf, &mut out);
    flush_cjk(&mut cjk, &mut out);
    out
}

pub fn stem(word: &str, lang: &str) -> String {
    let lang = lang_base(lang);
    let w = word.to_lowercase();
    match lang {
        "zh" | "ja" | "th" | "km" | "ar" | "he" => w,
        "de" => stem_de(&w),
        "fr" => stem_fr(&w),
        "es" | "pt" | "it" | "ca" | "gl" => stem_romance(&w),
        "ru" | "uk" | "be" => stem_cyrillic(&w),
        "nl" => strip_suffixes(&w, &["ingen", "isch", "heid", "lijk", "end", "ing", "en", "s"]),
        "sv" | "da" | "no" => strip_suffixes(&w, &["ning", "het", "are", "ade", "en", "er", "ar", "s"]),
        "pl" | "cs" | "sk" | "sl" | "hr" => {
            strip_suffixes(&w, &["owie", "ich", "ych", "ami", "ach", "em", "ie", "ów", "y", "a"])
        }
        "hu" => strip_suffixes(&w, &["okban", "nak", "nek", "ban", "ben", "ok", "ek", "k"]),
        "fi" => strip_suffixes(&w, &["ssa", "sta", "lla", "lta", "n", "t", "a"]),
        "tr" => strip_suffixes(&w, &["lerde", "larda", "ler", "lar", "in", "ın", "un", "ün"]),
        "el" => strip_suffixes(&w, &["ων", "ες", "ος", "η", "α"]),
        _ => stem_en(&w),
    }
}

/// Compact Porter-like English stemmer (enough for Java golden samples).
fn stem_en(w: &str) -> String {
    let mut s = w.to_string();
    if s.len() > 6 && s.ends_with("ational") {
        return format!("{}ate", &s[..s.len() - 7]);
    }
    if s.len() > 5 && s.ends_with("tional") {
        return format!("{}tion", &s[..s.len() - 6]);
    }
    for suf in ["ingly", "edly", "ness", "ment"] {
        if s.len() > suf.len() + 2 && s.ends_with(suf) {
            s.truncate(s.len() - suf.len());
            return s;
        }
    }
    if s.len() > 5 && s.ends_with("ing") {
        s.truncate(s.len() - 3);
        undouble(&mut s);
        return s;
    }
    if s.len() > 4 && s.ends_with("ed") && !s.ends_with("eed") {
        s.truncate(s.len() - 2);
        undouble(&mut s);
        return s;
    }
    if s.len() > 4 && s.ends_with("ies") {
        s.truncate(s.len() - 3);
        s.push('y');
        return s;
    }
    if s.len() > 3 && s.ends_with('s') && !s.ends_with("ss") && !s.ends_with("us") {
        s.pop();
    }
    s
}

fn undouble(s: &mut String) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let a = chars[chars.len() - 2];
        let b = chars[chars.len() - 1];
        if a == b && a.is_ascii_alphabetic() && !matches!(a, 'a' | 'e' | 'i' | 'o' | 'u') {
            s.pop();
        }
    }
}

fn stem_de(w: &str) -> String {
    strip_suffixes(
        w,
        &["ungen", "heit", "keit", "lich", "isch", "ung", "end", "ern", "en", "er", "st", "s"],
    )
}

fn stem_fr(w: &str) -> String {
    strip_suffixes(w, &["iquement", "ment", "tion", "eaux", "aux", "ées", "ent", "ons", "ez", "es", "e"])
}

fn stem_romance(w: &str) -> String {
    strip_suffixes(w, &["mente", "ción", "zione", "ando", "iendo", "ções", "es", "s", "a", "o"])
}

fn stem_cyrillic(w: &str) -> String {
    strip_suffixes(
        w,
        &["ами", "ями", "ого", "ему", "ых", "их", "ов", "ев", "ей", "ам", "ям", "ах", "ях", "ы", "и", "а", "я", "у", "ю"],
    )
}

fn strip_suffixes(w: &str, suffixes: &[&str]) -> String {
    for suf in suffixes {
        if w.len() > suf.len() + 2 && w.ends_with(suf) {
            return w[..w.len() - suf.len()].to_string();
        }
    }
    w.to_string()
}

pub fn lang_base(lang: &str) -> &str {
    lang.split(['-', '_']).next().unwrap_or(lang)
}

pub fn word_count(text: &str, lang: &str) -> usize {
    let lang = lang_base(lang);
    if matches!(lang, "zh" | "ja") {
        return text
            .chars()
            .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
            .count();
    }
    tokenize(text, lang).len()
}

pub fn tokenizer_id(lang: &str) -> &'static str {
    match lang_base(lang) {
        "en" => "lucene-en",
        "de" => "lucene-de",
        "fr" => "lucene-fr",
        "es" => "lucene-es",
        "pt" => "lucene-pt",
        "it" => "lucene-it",
        "ru" => "lucene-ru",
        "zh" | "ja" => "lucene-cjk",
        "ar" => "lucene-ar",
        other if !other.is_empty() => "lucene-default",
        _ => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_stems_running() {
        assert_eq!(stem("running", "en"), "run");
        assert_eq!(stem("worlds", "en"), "world");
    }

    #[test]
    fn cjk_overlapping_bigrams() {
        let t = tokenize("汉字词", "zh");
        assert_eq!(t.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["汉字", "字词"]);
    }

    #[test]
    fn tokenizer_matrix_covers_lucene_langs() {
        for lang in ["en", "de", "fr", "zh", "ja", "ru", "ar"] {
            assert!(tokenizer_id(lang).starts_with("lucene"));
        }
    }
}
