//! Java `LuceneTurkishTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneTurkishTokenizer;

impl Tokenizer for LuceneTurkishTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneTurkishTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["tr"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::turkish(w), stopwords::TR)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::turkish(w), stopwords::TR)
    }
}
