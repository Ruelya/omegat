//! Java `HunspellTokenizer` — stem via the Hunspell affix table when present.
use super::engine;
use super::{StemmingMode, Token, Tokenizer};

pub struct HunspellTokenizer;

impl Tokenizer for HunspellTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.HunspellTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["*"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(text, mode, |w, _| w.to_lowercase(), &[])
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(text, mode, |w, _| w.to_lowercase(), &[])
    }
}
