//! Java `LuceneNorwegianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneNorwegianTokenizer;

impl Tokenizer for LuceneNorwegianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneNorwegianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["nb"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::norwegian(w), stopwords::NO)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::norwegian(w), stopwords::NO)
    }
}
