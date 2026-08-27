//! Java `DefaultTokenizer` (WordIterator / BreakIterator, no stemming).
use super::engine;
use super::{StemmingMode, Token, Tokenizer};

pub struct DefaultTokenizer;

impl Tokenizer for DefaultTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.DefaultTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["*"]
    }
    fn tokenize_words(&self, text: &str, _mode: StemmingMode) -> Vec<String> {
        engine::default_words(text)
    }
    fn tokenize_tokens(&self, text: &str, _mode: StemmingMode) -> Vec<Token> {
        engine::default_word_tokens(text)
    }
}
