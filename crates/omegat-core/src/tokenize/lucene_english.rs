//! Java `LuceneEnglishTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneEnglishTokenizer;

impl Tokenizer for LuceneEnglishTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneEnglishTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["en"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, stem_en, stopwords::EN)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, stem_en, stopwords::EN)
    }
}

fn stem_en(word: &str, full: bool) -> String {
    let porter = stems::porter(word);
    if full {
        stems::snowball_en(&porter)
    } else {
        porter
    }
}
