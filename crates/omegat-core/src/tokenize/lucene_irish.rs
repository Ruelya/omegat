//! Java `LuceneIrishTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneIrishTokenizer;

impl Tokenizer for LuceneIrishTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneIrishTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ga"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::irish(w), stopwords::GA)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::irish(w), stopwords::GA)
    }
}
