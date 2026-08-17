use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub stem: String,
}

pub fn tokenize(text: &str, lang: &str) -> Vec<Token> {
    let lower = text.to_lowercase();
    let words = lower.unicode_words();
    words
        .map(|w| Token {
            stem: stem(w, lang),
            text: w.to_string(),
        })
        .collect()
}

pub fn stem(word: &str, lang: &str) -> String {
    let lang = lang.to_ascii_lowercase();
    let w = word.to_lowercase();
    if lang.starts_with("zh") || lang.starts_with("ja") || lang.starts_with("th") {
        return w;
    }
    // Light suffix stemmer approximating Lucene snowball for P1; P5 documents remaining gap.
    for suf in ["ingly", "edly", "ness", "ment", "tion", "sion", "ing", "ed", "es", "s", "ly"] {
        if w.len() > suf.len() + 2 && w.ends_with(suf) {
            return w[..w.len() - suf.len()].to_string();
        }
    }
    w
}

pub fn word_count(text: &str, lang: &str) -> usize {
    if lang.to_ascii_lowercase().starts_with("zh")
        || lang.to_ascii_lowercase().starts_with("ja")
    {
        return text.chars().filter(|c| !c.is_whitespace()).count();
    }
    tokenize(text, lang).len()
}
