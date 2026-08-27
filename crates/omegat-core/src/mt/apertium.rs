//! Java `ApertiumTranslate`.

use serde_json::Value;

pub const ID: &str = "apertium";
pub const ENDPOINT: &str = "https://www.apertium.org/apy/translate";

pub fn parse(v: &Value) -> Option<String> {
    v.pointer("/responseData/translatedText")
        .or_else(|| v.pointer("/translatedText"))
        .and_then(|x| x.as_str())
        .map(str::to_string)
}
