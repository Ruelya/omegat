//! Java `LuceneBulgarianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneBulgarianTokenizer;

impl Tokenizer for LuceneBulgarianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneBulgarianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["bg"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::bulgarian(w), stopwords::BG)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::bulgarian(w), stopwords::BG)
    }
}
