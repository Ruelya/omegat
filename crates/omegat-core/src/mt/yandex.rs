//! Java `YandexCloudTranslate`.

use serde_json::Value;

pub const ID: &str = "yandex";
pub const ENDPOINT: &str = "https://translate.api.cloud.yandex.net/translate/v2/translate";

pub fn parse(v: &Value) -> Option<String> {
    v.pointer("/translations/0/text")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}

pub fn auth_headers(iam: &str) -> Result<Vec<(String, String)>, String> {
    if iam.is_empty() && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
        return Err("Yandex IAM token missing".into());
    }
    Ok(vec![("Authorization".into(), format!("Bearer {iam}"))])
}
