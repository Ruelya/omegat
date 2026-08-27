//! One module per Java `*Tokenizer`. Shared event-like engine lives in `engine`.

mod default;
pub(crate) mod engine;
mod hunspell;
mod lucene_arabic;
mod lucene_armenian;
mod lucene_basque;
mod lucene_brazilian;
mod lucene_bulgarian;
mod lucene_catalan;
mod lucene_cjk;
mod lucene_czech;
mod lucene_danish;
mod lucene_dutch;
mod lucene_english;
mod lucene_finnish;
mod lucene_french;
mod lucene_galician;
mod lucene_german;
mod lucene_greek;
mod lucene_hindi;
mod lucene_hungarian;
mod lucene_indonesian;
mod lucene_irish;
mod lucene_italian;
mod lucene_japanese;
mod lucene_latvian;
mod lucene_norwegian;
mod lucene_persian;
mod lucene_polish;
mod lucene_portuguese;
mod lucene_romanian;
mod lucene_russian;
mod lucene_smart_chinese;
mod lucene_spanish;
mod lucene_swedish;
mod lucene_thai;
mod lucene_turkish;
mod stems;
mod stopwords;

pub use default::DefaultTokenizer;
pub use hunspell::HunspellTokenizer;
pub use lucene_arabic::LuceneArabicTokenizer;
pub use lucene_armenian::LuceneArmenianTokenizer;
pub use lucene_basque::LuceneBasqueTokenizer;
pub use lucene_brazilian::LuceneBrazilianTokenizer;
pub use lucene_bulgarian::LuceneBulgarianTokenizer;
pub use lucene_catalan::LuceneCatalanTokenizer;
pub use lucene_cjk::LuceneCJKTokenizer;
pub use lucene_czech::LuceneCzechTokenizer;
pub use lucene_danish::LuceneDanishTokenizer;
pub use lucene_dutch::LuceneDutchTokenizer;
pub use lucene_english::LuceneEnglishTokenizer;
pub use lucene_finnish::LuceneFinnishTokenizer;
pub use lucene_french::LuceneFrenchTokenizer;
pub use lucene_galician::LuceneGalicianTokenizer;
pub use lucene_german::LuceneGermanTokenizer;
pub use lucene_greek::LuceneGreekTokenizer;
pub use lucene_hindi::LuceneHindiTokenizer;
pub use lucene_hungarian::LuceneHungarianTokenizer;
pub use lucene_irish::LuceneIrishTokenizer;
pub use lucene_indonesian::LuceneIndonesianTokenizer;
pub use lucene_italian::LuceneItalianTokenizer;
pub use lucene_japanese::LuceneJapaneseTokenizer;
pub use lucene_latvian::LuceneLatvianTokenizer;
pub use lucene_norwegian::LuceneNorwegianTokenizer;
pub use lucene_persian::LucenePersianTokenizer;
pub use lucene_polish::LucenePolishTokenizer;
pub use lucene_portuguese::LucenePortugueseTokenizer;
pub use lucene_romanian::LuceneRomanianTokenizer;
pub use lucene_russian::LuceneRussianTokenizer;
pub use lucene_smart_chinese::LuceneSmartChineseTokenizer;
pub use lucene_spanish::LuceneSpanishTokenizer;
pub use lucene_swedish::LuceneSwedishTokenizer;
pub use lucene_thai::LuceneThaiTokenizer;
pub use lucene_turkish::LuceneTurkishTokenizer;

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub stem: String,
}

impl Token {
    /// Java `Token.equals`: hash of `stripAmpersand(text)` only.
    pub fn java_equals(&self, other: &Self) -> bool {
        java_string_hash(&strip_ampersand(&self.text)) == java_string_hash(&strip_ampersand(&other.text))
    }

    pub fn java_deep_equals(&self, offset: usize, other: &Self, other_offset: usize) -> bool {
        self.java_equals(other) && offset == other_offset && self.text.chars().count() == other.text.chars().count()
    }
}

/// Java `String.hashCode` over UTF-16 code units.
pub fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    h
}

fn strip_ampersand(s: &str) -> String {
    s.replace('&', "")
}

/// Attach display offsets the way Java `Token(text, offset)` does.
pub fn with_offsets(text: &str, tokens: &[Token]) -> Vec<(Token, usize)> {
    let mut from = 0;
    let mut out = Vec::new();
    for t in tokens {
        if let Some(pos) = text[from..].find(&t.text) {
            let abs = from + pos;
            out.push((t.clone(), abs));
            from = abs + t.text.len();
        } else if let Some(pos) = text.find(&t.text) {
            out.push((t.clone(), pos));
        } else {
            out.push((t.clone(), from));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemmingMode {
    None,
    Glossary,
    GlossaryFull,
    Matching,
    MatchingFull,
}

impl StemmingMode {
    pub fn parse(name: &str) -> Self {
        match name {
            "GLOSSARY" => Self::Glossary,
            "GLOSSARY_FULL" => Self::GlossaryFull,
            "MATCHING" => Self::Matching,
            "MATCHING_FULL" => Self::MatchingFull,
            _ => Self::None,
        }
    }
    pub fn stems_allowed(self) -> bool {
        !matches!(self, Self::None)
    }
    pub fn stop_words(self) -> bool {
        matches!(self, Self::Matching | Self::MatchingFull)
    }
    pub fn filter_digits(self) -> bool {
        !matches!(self, Self::Glossary | Self::GlossaryFull)
    }
    pub fn full(self) -> bool {
        matches!(self, Self::GlossaryFull | Self::MatchingFull)
    }
}

pub trait Tokenizer: Send + Sync {
    fn id(&self) -> &'static str;
    fn languages(&self) -> &'static [&'static str];
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String>;
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token>;
}

fn all_tokenizers() -> Vec<&'static dyn Tokenizer> {
    vec![
        &DefaultTokenizer,
        &HunspellTokenizer,
        &LuceneArabicTokenizer,
        &LuceneArmenianTokenizer,
        &LuceneBasqueTokenizer,
        &LuceneBrazilianTokenizer,
        &LuceneBulgarianTokenizer,
        &LuceneCatalanTokenizer,
        &LuceneCJKTokenizer,
        &LuceneCzechTokenizer,
        &LuceneDanishTokenizer,
        &LuceneDutchTokenizer,
        &LuceneEnglishTokenizer,
        &LuceneFinnishTokenizer,
        &LuceneFrenchTokenizer,
        &LuceneGalicianTokenizer,
        &LuceneGermanTokenizer,
        &LuceneGreekTokenizer,
        &LuceneHindiTokenizer,
        &LuceneHungarianTokenizer,
        &LuceneIndonesianTokenizer,
        &LuceneIrishTokenizer,
        &LuceneItalianTokenizer,
        &LuceneJapaneseTokenizer,
        &LuceneLatvianTokenizer,
        &LuceneNorwegianTokenizer,
        &LucenePersianTokenizer,
        &LucenePolishTokenizer,
        &LucenePortugueseTokenizer,
        &LuceneRomanianTokenizer,
        &LuceneRussianTokenizer,
        &LuceneSmartChineseTokenizer,
        &LuceneSpanishTokenizer,
        &LuceneSwedishTokenizer,
        &LuceneThaiTokenizer,
        &LuceneTurkishTokenizer,
    ]
}

pub fn for_class(class: &str) -> &'static dyn Tokenizer {
    let short = class.rsplit('.').next().unwrap_or(class);
    all_tokenizers()
        .into_iter()
        .find(|t| t.id() == class || t.id().ends_with(short))
        .unwrap_or(&DefaultTokenizer)
}

pub fn for_lang(lang: &str) -> &'static dyn Tokenizer {
    let base = lang_base(lang);
    match base {
        "zh" | "ja" | "ko" => &LuceneCJKTokenizer,
        _ => all_tokenizers()
            .into_iter()
            .find(|t| t.languages().iter().any(|l| *l == base || *l == lang))
            .unwrap_or(&DefaultTokenizer),
    }
}

/// Language-aware tokenization used by matching / glossary.
/// CJK keeps Lucene CJK overlapping bigrams so G1 goldens stay `assert_eq`.
pub fn tokenize(text: &str, lang: &str) -> Vec<Token> {
    let tok = for_lang(lang);
    tok.tokenize_tokens(text, StemmingMode::None)
        .into_iter()
        .map(|t| Token {
            stem: stem(&t.text, lang),
            text: t.text,
        })
        .collect()
}

pub fn tokenize_words(text: &str, class: &str, mode: StemmingMode) -> Vec<String> {
    for_class(class).tokenize_words(text, mode)
}

/// Java `ITokenizer.tokenizeWords` (Token[]): analyzer terms only, no surface pair.
pub fn tokenize_word_tokens(text: &str, class: &str, mode: StemmingMode) -> Vec<String> {
    if class.contains("LuceneCJK") || class.contains("DefaultTokenizer") {
        return for_class(class).tokenize_words(text, mode);
    }
    for_class(class)
        .tokenize_tokens(text, mode)
        .into_iter()
        .filter(|t| !mode.stems_allowed() || t.text == t.stem)
        .map(|t| t.text)
        .collect()
}

/// Java `DefaultTokenizer.tokenizeVerbatim`: every WordIterator token, including
/// numbers, tags, and whitespace.
pub fn tokenize_verbatim(text: &str) -> Vec<String> {
    engine::word_iterator_surfaces(text)
        .into_iter()
        .map(|s| s.text.to_string())
        .collect()
}

/// Java `DefaultTokenizer.isContains`.
pub fn is_contains(tokens: &[String], tok: &str) -> bool {
    tokens.iter().any(|t| t == tok)
}

/// Java `DefaultTokenizer.isContainsAll`. `inexact=true` ignores order / gaps.
pub fn is_contains_all(tokens: &[String], find: &[String], inexact: bool) -> bool {
    if find.is_empty() {
        return true;
    }
    if inexact {
        find.iter().all(|f| tokens.iter().any(|t| t == f))
    } else {
        tokens.windows(find.len()).any(|w| w == find)
    }
}

/// Java `DefaultTokenizer.searchAll`.
pub fn search_all(tokens: &[String], find: &[String], inexact: bool) -> Vec<Vec<String>> {
    if find.is_empty() {
        return vec![];
    }
    if inexact {
        let mut uniq = Vec::new();
        for f in find {
            if !uniq.iter().any(|u| u == f) {
                uniq.push(f.clone());
            }
        }
        let mut hits = Vec::new();
        for t in tokens {
            if uniq.iter().any(|f| f == t) {
                hits.push(t.clone());
            }
        }
        if hits.len() < find.len() {
            return vec![];
        }
        vec![hits]
    } else {
        let mut out = Vec::new();
        if find.len() == 1 {
            for t in tokens {
                if t == &find[0] {
                    out.push(vec![t.clone()]);
                }
            }
            return out;
        }
        let mut i = 0;
        while i + find.len() <= tokens.len() {
            if tokens[i..i + find.len()] == find[..] {
                out.push(find.to_vec());
                i += find.len();
            } else {
                i += 1;
            }
        }
        out
    }
}

pub fn stem(word: &str, lang: &str) -> String {
    let lang = lang_base(lang);
    match lang {
        "en" => stems::porter(word),
        "de" => stems::german_lucene30(word),
        "fr" => stems::french(word, false),
        "es" => stems::spanish_light(word),
        "pt" => stems::portuguese_light(word),
        "it" => stems::italian_light(word),
        "nl" => stems::dutch(word),
        "ru" | "uk" | "be" => stems::russian(word),
        "tr" => stems::turkish(word),
        "ar" => stems::arabic(word),
        "fa" => stems::persian(word),
        "hi" => stems::hindi(word),
        "zh" | "ja" | "th" | "km" | "he" => word.to_lowercase(),
        "sv" => stems::swedish(word),
        "da" => stems::danish(word),
        "no" | "nb" | "nn" => stems::norwegian(word),
        "pl" => stems::polish(word),
        "cs" => stems::czech(word),
        "sk" | "sl" | "hr" => stems::czech(word),
        "hu" => stems::hungarian(word),
        "fi" => stems::finnish(word),
        "el" => stems::greek(word),
        "hy" => stems::armenian(word),
        "eu" => stems::basque(word),
        "ca" => stems::catalan(word),
        "gl" => stems::galician(word),
        "ga" => stems::irish(word),
        "lv" => stems::latvian(word),
        "bg" => stems::bulgarian(word),
        "id" => stems::indonesian(word),
        "ro" => stems::romanian(word),
        _ => stems::porter(word),
    }
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
    for_lang(lang).id()
}

pub fn registered_lucene_tokenizers() -> Vec<&'static str> {
    all_tokenizers()
        .into_iter()
        .map(|t| t.id())
        .filter(|id| id.contains("Lucene"))
        .collect()
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
            assert!(tokenizer_id(lang).contains("Lucene"), "{}", tokenizer_id(lang));
        }
        assert_eq!(registered_lucene_tokenizers().len(), 34);
    }

    #[test]
    fn english_glossary_pairs_stem_and_surface() {
        let w = LuceneEnglishTokenizer.tokenize_words(
            "The quick, brown <x0/> jumped over 1 \"lazy\" dog.",
            StemmingMode::Glossary,
        );
        assert!(w.contains(&"jump".into()) && w.contains(&"jumped".into()));
        assert!(w.contains(&"lazi".into()) && w.contains(&"lazy".into()));
    }
}
