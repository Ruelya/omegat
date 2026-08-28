//! Java `LuceneCzechTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneCzechTokenizer;

impl Tokenizer for LuceneCzechTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneCzechTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["cs"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::czech(w), stopwords::CS)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::czech(w), stopwords::CS)
    }
}
