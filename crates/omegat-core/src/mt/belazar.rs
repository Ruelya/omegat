//! Java `BelazarTranslate`.

use serde_json::Value;

pub const ID: &str = "belazar";
pub const ENDPOINT: &str = "http://www.belazar.by/translate";

pub fn parse(v: &Value) -> Option<String> {
    v.get("text")
        .and_then(|x| x.as_str())
        .or_else(|| v.as_str())
        .map(str::to_string)
}
