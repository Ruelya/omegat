//! Java `IBMWatsonTranslate`.

use serde_json::Value;

pub const ID: &str = "ibmwatson";
pub const ENDPOINT: &str =
    "https://api.us-south.language-translator.watson.cloud.ibm.com/v3/translate";

pub fn parse(v: &Value) -> Option<String> {
    v.pointer("/translations/0/translation")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}

pub fn auth_headers(login: &str, password: &str) -> Result<Vec<(String, String)>, String> {
    if password.is_empty() && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
        return Err("IBM Watson API key missing".into());
    }
    let token = super::base64_basic(&format!("{login}:{password}"));
    Ok(vec![
        ("Authorization".into(), format!("Basic {token}")),
        ("X-Watson-Learning-Opt-Out".into(), "true".into()),
        ("Accept".into(), "application/json".into()),
    ])
}
