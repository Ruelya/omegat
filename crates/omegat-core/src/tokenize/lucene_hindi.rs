//! Java `LuceneHindiTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneHindiTokenizer;

impl Tokenizer for LuceneHindiTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneHindiTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["hi"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::hindi(w), stopwords::HI)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::hindi(w), stopwords::HI)
    }
}
