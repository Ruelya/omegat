//! Java `LuceneRussianTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneRussianTokenizer;

impl Tokenizer for LuceneRussianTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneRussianTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["ru"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::russian(w), stopwords::RU)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::russian(w), stopwords::RU)
    }
}
