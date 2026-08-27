//! Java `LuceneBasqueTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneBasqueTokenizer;

impl Tokenizer for LuceneBasqueTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneBasqueTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["eu"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::basque(w), stopwords::EU)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::basque(w), stopwords::EU)
    }
}
