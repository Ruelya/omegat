//! Java `LuceneDutchTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneDutchTokenizer;

impl Tokenizer for LuceneDutchTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneDutchTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["nl"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::dutch(w), stopwords::NL)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::dutch(w), stopwords::NL)
    }
}
