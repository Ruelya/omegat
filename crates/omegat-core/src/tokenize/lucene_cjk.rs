//! Java `LuceneCJKTokenizer`.
use super::engine;
use super::{StemmingMode, Token, Tokenizer};

pub struct LuceneCJKTokenizer;

impl Tokenizer for LuceneCJKTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneCJKTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["zh", "ja", "ko"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode).into_iter().map(|t| t.text).collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        let tokens = engine::cjk_bigrams(text, true);
        if mode.filter_digits() {
            tokens.into_iter().filter(|t| engine::accept_token(&t.text, true)).collect()
        } else {
            tokens
        }
    }
}
