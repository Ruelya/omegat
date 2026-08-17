//! Java `LuceneLatvianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneLatvianTokenizer;

impl Tokenizer for LuceneLatvianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneLatvianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["lv"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::identity(w), stopwords::GENERIC)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::identity(w), stopwords::GENERIC)
    }
}
