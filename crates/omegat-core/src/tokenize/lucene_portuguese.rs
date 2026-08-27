//! Java `LucenePortugueseTokenizer`.
use super::engine;
use super::stems;
use super::stopwords;
use super::{StemmingMode, Token, Tokenizer};

pub struct LucenePortugueseTokenizer;

impl Tokenizer for LucenePortugueseTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LucenePortugueseTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["pt"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        engine::lucene_words_to_strings(
            text,
            mode,
            |w, _full| stems::portuguese_light(w),
            stopwords::PT,
        )
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        engine::lucene_tokens(
            text,
            mode,
            |w, _full| stems::portuguese_light(w),
            stopwords::PT,
        )
    }
}
