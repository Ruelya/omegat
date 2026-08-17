//! Java `LuceneCatalanTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneCatalanTokenizer;

impl Tokenizer for LuceneCatalanTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneCatalanTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ca"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::catalan(w), stopwords::CA)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::catalan(w), stopwords::CA)
    }
}
