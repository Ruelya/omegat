//! Java `LuceneBrazilianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneBrazilianTokenizer;

impl Tokenizer for LuceneBrazilianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneBrazilianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["pt-br"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::brazilian(w), stopwords::BR)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::brazilian(w), stopwords::BR)
    }
}
