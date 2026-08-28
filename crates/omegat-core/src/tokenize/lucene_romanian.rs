//! Java `LuceneRomanianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneRomanianTokenizer;

impl Tokenizer for LuceneRomanianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneRomanianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ro"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::romanian(w), stopwords::RO)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::romanian(w), stopwords::RO)
    }
}
