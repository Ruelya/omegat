//! Java `LuceneSwedishTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneSwedishTokenizer;

impl Tokenizer for LuceneSwedishTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneSwedishTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["sv"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _full| stems::swedish(w), stopwords::SV)
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _full| stems::swedish(w), stopwords::SV)
    }
}
