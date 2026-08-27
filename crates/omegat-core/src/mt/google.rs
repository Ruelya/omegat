//! Java `Google2Translator` — Translate API v2.

use serde_json::Value;

pub const ID: &str = "google";
pub const ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

pub fn parse(v: &Value) -> Option<String> {
    v.pointer("/data/translations/0/translatedText")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}

pub fn auth_headers(key_present: bool) -> Result<Vec<(String, String)>, String> {
    if !key_present && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
        return Err("google.api.key missing".into());
    }
    Ok(vec![("X-HTTP-Method-Override".into(), "GET".into())])
}
