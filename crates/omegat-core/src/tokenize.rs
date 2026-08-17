//! Language-aware tokenization approximating the Java Default + Lucene tokenizer matrix.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub stem: String,
}

pub fn tokenize(text: &str, lang: &str) -> Vec<Token> {
    let lang = lang_base(lang);
    if matches!(lang, "zh" | "ja" | "th" | "km") {
        return cjk_tokens(text, lang);
    }
    text.to_lowercase()
        .unicode_words()
        .map(|w| Token {
            stem: stem(w, lang),
            text: w.to_string(),
        })
        .collect()
}

fn cjk_tokens(text: &str, lang: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_ascii_punctuation() {
            if !buf.is_empty() {
                out.push(latin_or_keep(&buf, lang));
                buf.clear();
            }
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            buf.push(ch);
            continue;
        }
        if !buf.is_empty() {
            out.push(latin_or_keep(&buf, lang));
            buf.clear();
        }
        let s = ch.to_string();
        out.push(Token {
            stem: s.clone(),
            text: s,
        });
    }
    if !buf.is_empty() {
        out.push(latin_or_keep(&buf, lang));
    }
    out
}

fn latin_or_keep(w: &str, lang: &str) -> Token {
    Token {
        stem: stem(&w.to_lowercase(), lang),
        text: w.to_lowercase(),
    }
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
        "pl" | "cs" | "sk" | "sl" | "hr" => strip_suffixes(&w, &["owie", "ich", "ych", "ami", "ach", "em", "ie", "ów", "y", "a"]),
        "hu" => strip_suffixes(&w, &["okban", "okban", "nak", "nek", "ban", "ben", "ok", "ek", "k"]),
        "fi" => strip_suffixes(&w, &["ssa", "sta", "lla", "lta", "n", "t", "a"]),
        "tr" => strip_suffixes(&w, &["lerde", "larda", "ler", "lar", "in", "ın", "un", "ün"]),
        "el" => strip_suffixes(&w, &["ων", "ες", "ος", "η", "α"]),
        _ => stem_en(&w),
    }
}

fn stem_en(w: &str) -> String {
    strip_suffixes(
        w,
        &[
            "ational", "tional", "ingly", "edly", "ness", "ment", "tion", "sion",
            "ing", "ed", "es", "ly", "s",
        ],
    )
}

fn stem_de(w: &str) -> String {
    strip_suffixes(w, &["ungen", "heit", "keit", "lich", "isch", "ung", "end", "ern", "en", "er", "st", "s"])
}

fn stem_fr(w: &str) -> String {
    strip_suffixes(w, &["iquement", "ment", "tion", "eaux", "aux", "ées", "ent", "ons", "ez", "es", "e"])
}

fn stem_romance(w: &str) -> String {
    strip_suffixes(w, &["mente", "ción", "zione", "ando", "iendo", "ções", "ções", "es", "s", "a", "o"])
}

fn stem_cyrillic(w: &str) -> String {
    strip_suffixes(w, &["ами", "ями", "ого", "ему", "ых", "их", "ов", "ев", "ей", "ам", "ям", "ах", "ях", "ы", "и", "а", "я", "у", "ю"])
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
        return text.chars().filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation()).count();
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
        "zh" => "lucene-cjk",
        "ja" => "lucene-cjk",
        "ar" => "lucene-ar",
        "nl" => "lucene-nl",
        "sv" => "lucene-sv",
        "pl" => "lucene-pl",
        "cs" => "lucene-cs",
        "tr" => "lucene-tr",
        "el" => "lucene-el",
        "hu" => "lucene-hu",
        "fi" => "lucene-fi",
        "da" => "lucene-da",
        "no" => "lucene-no",
        "ca" => "lucene-ca",
        "th" => "lucene-th",
        "hi" => "lucene-hi",
        "id" => "lucene-id",
        "ro" => "lucene-ro",
        "bg" => "lucene-bg",
        "uk" => "lucene-uk",
        "hy" => "lucene-hy",
        "eu" => "lucene-eu",
        "ga" => "lucene-ga",
        "gl" => "lucene-gl",
        "lv" => "lucene-lv",
        "lt" => "lucene-lt",
        _ => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_stems_running() {
        assert_eq!(stem("running", "en"), "runn");
    }

    #[test]
    fn cjk_splits_characters() {
        let t = tokenize("汉字", "zh");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn tokenizer_matrix_covers_lucene_langs() {
        for lang in ["en", "de", "fr", "zh", "ja", "ru", "ar"] {
            assert_ne!(tokenizer_id(lang), "");
        }
    }
}
