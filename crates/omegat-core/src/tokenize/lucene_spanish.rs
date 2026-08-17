//! Java `LuceneSpanishTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneSpanishTokenizer;

impl Tokenizer for LuceneSpanishTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneSpanishTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["es"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::spanish_light(w), stopwords::ES)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::spanish_light(w), stopwords::ES)
    }
}
