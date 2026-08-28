//! Java `MyMemoryHumanTranslate`.

use serde_json::Value;

pub const ID: &str = "mymemory-human";
pub const ENDPOINT: &str = "https://api.mymemory.translated.net/get";

pub fn parse(v: &Value) -> Option<String> {
    v.pointer("/responseData/translatedText")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}
