use crate::languagetool::http_exchange;
use omegat_ipc::MtSuggestionDto;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MtEngine {
    pub id: String,
    pub name: String,
    pub endpoint: String,
}

pub fn engines() -> Vec<MtEngine> {
    vec![
        MtEngine { id: "google".into(), name: "Google Translate".into(), endpoint: "https://translation.googleapis.com/language/translate/v2".into() },
        MtEngine { id: "ibmwatson".into(), name: "IBM Watson".into(), endpoint: "https://api.us-south.language-translator.watson.cloud.ibm.com/v3/translate".into() },
        MtEngine { id: "mymemory".into(), name: "MyMemory Machine".into(), endpoint: "https://api.mymemory.translated.net/get".into() },
        MtEngine { id: "mymemory-human".into(), name: "MyMemory Human".into(), endpoint: "https://api.mymemory.translated.net/get".into() },
        MtEngine { id: "apertium".into(), name: "Apertium".into(), endpoint: "https://www.apertium.org/apy/translate".into() },
        MtEngine { id: "yandex".into(), name: "Yandex Cloud".into(), endpoint: "https://translate.api.cloud.yandex.net/translate/v2/translate".into() },
        MtEngine { id: "belazar".into(), name: "Belazar".into(), endpoint: "http://www.belazar.by/translate".into() },
    ]
}

#[derive(Default)]
pub struct MtCache {
    inner: Mutex<HashMap<String, String>>,
}

impl MtCache {
    pub fn get(&self, key: &str) -> Option<String> {
        self.inner.lock().ok()?.get(key).cloned()
    }
    pub fn put(&self, key: String, value: String) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(key, value);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MtCreds {
    pub google_key: Option<String>,
    pub ibm_login: Option<String>,
    pub ibm_password: Option<String>,
    pub yandex_iam: Option<String>,
    pub mymemory_key: Option<String>,
}

impl MtCreds {
    pub fn from_extra(extra: &std::collections::HashMap<String, String>) -> Self {
        Self {
            google_key: extra.get("mt.google.key").cloned().or_else(|| std::env::var("OMEGAT_GOOGLE_KEY").ok()),
            ibm_login: extra.get("mt.ibmwatson.login").cloned().or_else(|| extra.get("mt.ibmwatson.key").cloned()),
            ibm_password: extra.get("mt.ibmwatson.password").cloned(),
            yandex_iam: extra.get("mt.yandex.key").cloned().or_else(|| std::env::var("OMEGAT_YANDEX_IAM").ok()),
            mymemory_key: extra.get("mt.mymemory.key").cloned(),
        }
    }
}

/// Java connector auth headers (no secrets in the values used by tests).
pub fn auth_headers(engine: &str, creds: &MtCreds) -> Result<Vec<(String, String)>, String> {
    match engine {
        "google" => {
            if creds.google_key.as_deref().unwrap_or("").is_empty() && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
                return Err("google.api.key missing".into());
            }
            Ok(vec![("X-HTTP-Method-Override".into(), "GET".into())])
        }
        "ibmwatson" => {
            let login = creds.ibm_login.clone().unwrap_or_else(|| "apikey".into());
            let pass = creds.ibm_password.clone().or_else(|| creds.ibm_login.clone()).unwrap_or_default();
            if pass.is_empty() && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
                return Err("IBM Watson API key missing".into());
            }
            let token = base64_basic(&format!("{login}:{pass}"));
            Ok(vec![
                ("Authorization".into(), format!("Basic {token}")),
                ("X-Watson-Learning-Opt-Out".into(), "true".into()),
                ("Accept".into(), "application/json".into()),
            ])
        }
        "yandex" => {
            let iam = creds.yandex_iam.clone().unwrap_or_default();
            if iam.is_empty() && std::env::var("OMEGAT_MT_FIXTURE_DIR").is_err() {
                return Err("Yandex IAM token missing".into());
            }
            Ok(vec![("Authorization".into(), format!("Bearer {iam}"))])
        }
        "mymemory" | "mymemory-human" | "apertium" | "belazar" => Ok(vec![]),
        other => Err(format!("unknown engine {other}")),
    }
}

fn base64_basic(s: &str) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let n = match b.len() - i {
            1 => 1,
            2 => 2,
            _ => 3,
        };
        let x = (b[i] as u32) << 16
            | (if n > 1 { b[i + 1] as u32 } else { 0 }) << 8
            | if n > 2 { b[i + 2] as u32 } else { 0 };
        out.push(T[((x >> 18) & 63) as usize] as char);
        out.push(T[((x >> 12) & 63) as usize] as char);
        out.push(if n > 1 { T[((x >> 6) & 63) as usize] as char } else { '=' });
        out.push(if n > 2 { T[(x & 63) as usize] as char } else { '=' });
        i += n;
    }
    out
}

pub fn translate(
    engine: &str,
    source: &str,
    source_lang: &str,
    target_lang: &str,
    cache: &MtCache,
) -> Result<MtSuggestionDto, String> {
    translate_with_creds(engine, source, source_lang, target_lang, cache, &MtCreds::default())
}

pub fn translate_with_creds(
    engine: &str,
    source: &str,
    source_lang: &str,
    target_lang: &str,
    cache: &MtCache,
    creds: &MtCreds,
) -> Result<MtSuggestionDto, String> {
    let key = format!("{engine}:{source_lang}:{target_lang}:{source}");
    if let Some(text) = cache.get(&key) {
        return Ok(MtSuggestionDto { engine: engine.into(), text });
    }
    if engine == "mock" {
        return Err("mock is not a production engine".into());
    }
    let _ = auth_headers(engine, creds)?;
    let text = dispatch(engine, source, source_lang, target_lang, creds)?;
    cache.put(key, text.clone());
    Ok(MtSuggestionDto { engine: engine.into(), text })
}

fn dispatch(engine: &str, source: &str, sl: &str, tl: &str, creds: &MtCreds) -> Result<String, String> {
    if let Ok(dir) = std::env::var("OMEGAT_MT_FIXTURE_DIR") {
        let recorded = std::path::Path::new(&dir).join(engine).join("recorded.json");
        let legacy = std::path::Path::new(&dir).join(format!("{engine}.json"));
        let path = if recorded.exists() { recorded } else { legacy };
        if path.exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            return parse_recorded_or_engine(engine, &raw);
        }
        return Err(format!("{engine} has no recorded fixture under {dir}"));
    }
    if std::env::var("OMEGAT_MT_NETWORK").ok().as_deref() != Some("1") {
        return Err(format!("{engine} requires network (OMEGAT_MT_NETWORK=1) or OMEGAT_MT_FIXTURE_DIR"));
    }
    let raw = match engine {
        "mymemory" | "mymemory-human" => {
            let url = format!(
                "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
                urlencoding::encode(source),
                urlencoding::encode(sl),
                urlencoding::encode(tl)
            );
            http_exchange("GET", &url, None)?
        }
        "apertium" => {
            let url = format!(
                "https://www.apertium.org/apy/translate?q={}&langpair={}|{}",
                urlencoding::encode(source),
                urlencoding::encode(sl),
                urlencoding::encode(tl)
            );
            http_exchange("GET", &url, None)?
        }
        "google" => {
            let key = creds
                .google_key
                .clone()
                .or_else(|| std::env::var("OMEGAT_GOOGLE_KEY").ok())
                .ok_or("OMEGAT_GOOGLE_KEY")?;
            let url = format!(
                "https://translation.googleapis.com/language/translate/v2?key={}",
                urlencoding::encode(&key)
            );
            let body = format!(
                "{{\"q\":\"{}\",\"source\":\"{}\",\"target\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange("POST", &url, Some(("application/json", &body)))?
        }
        "ibmwatson" => {
            let url = std::env::var("OMEGAT_IBM_URL").unwrap_or_else(|_| engines()[1].endpoint.clone());
            let body = format!(
                "{{\"text\":[\"{}\"],\"source\":\"{}\",\"target\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange("POST", &url, Some(("application/json", &body)))?
        }
        "yandex" => {
            let url = std::env::var("OMEGAT_YANDEX_URL").unwrap_or_else(|_| engines()[5].endpoint.clone());
            let body = format!(
                "{{\"texts\":[\"{}\"],\"sourceLanguageCode\":\"{}\",\"targetLanguageCode\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange("POST", &url, Some(("application/json", &body)))?
        }
        "belazar" => {
            let url = format!(
                "http://www.belazar.by/translate?text={}&sl={}&tl={}",
                urlencoding::encode(source), sl, tl
            );
            http_exchange("GET", &url, None)?
        }
        other => return Err(format!("unknown engine {other}")),
    };
    parse_recorded_or_engine(engine, &raw)
}

pub fn parse_recorded_or_engine(engine: &str, raw: &str) -> Result<String, String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(resp) = v.get("response") {
            if let Some(err) = parse_error_body(engine, resp) {
                return Err(err);
            }
            return parse_engine(engine, &resp.to_string());
        }
    }
    if let Some(err) = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| parse_error_body(engine, &v))
    {
        return Err(err);
    }
    parse_engine(engine, raw)
}

pub fn parse_error_body(engine: &str, v: &serde_json::Value) -> Option<String> {
    let msg = v
        .pointer("/error/message")
        .or_else(|| v.pointer("/error/error"))
        .or_else(|| v.get("errorMessage"))
        .and_then(|x| x.as_str())?;
    Some(format!("{engine} error: {msg}"))
}

pub fn parse_engine(engine: &str, raw: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.to_string()));
    let text = match engine {
        "google" => v.pointer("/data/translations/0/translatedText").and_then(|x| x.as_str()),
        "ibmwatson" => v.pointer("/translations/0/translation").and_then(|x| x.as_str()),
        "mymemory" | "mymemory-human" => v.pointer("/responseData/translatedText").and_then(|x| x.as_str()),
        "apertium" => v.pointer("/responseData/translatedText").or_else(|| v.pointer("/translatedText")).and_then(|x| x.as_str()),
        "yandex" => v.pointer("/translations/0/text").and_then(|x| x.as_str()),
        "belazar" => v.get("text").and_then(|x| x.as_str()).or_else(|| v.as_str()),
        _ => v.get("text").and_then(|x| x.as_str()),
    };
    text.map(|s| html_escape::decode_html_entities(s).into_owned())
        .ok_or_else(|| format!("{engine} response missing translation"))
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_engine_fixture() {
        let samples = [
            ("google", r#"{"data":{"translations":[{"translatedText":"Bonjour"}]}}"#),
            ("ibmwatson", r#"{"translations":[{"translation":"Bonjour"}]}"#),
            ("mymemory", r#"{"responseData":{"translatedText":"Bonjour"}}"#),
            ("mymemory-human", r#"{"responseData":{"translatedText":"Bonjour"}}"#),
            ("apertium", r#"{"responseData":{"translatedText":"Bonjour"}}"#),
            ("yandex", r#"{"translations":[{"text":"Bonjour"}]}"#),
            ("belazar", r#"{"text":"Bonjour"}"#),
        ];
        for (id, raw) in samples {
            assert_eq!(parse_engine(id, raw).unwrap(), "Bonjour", "{id}");
        }
    }

    #[test]
    fn offline_without_fixture_is_error() {
        std::env::remove_var("OMEGAT_MT_FIXTURE_DIR");
        std::env::remove_var("OMEGAT_MT_NETWORK");
        let cache = MtCache::default();
        let err = translate("mymemory", "Hi", "en", "fr", &cache).unwrap_err();
        assert!(
            err.contains("OMEGAT_MT_NETWORK") || err.contains("FIXTURE") || err.contains("no recorded"),
            "{err}"
        );
        let g = translate("google", "Hi", "en", "fr", &cache).unwrap_err();
        assert!(g.contains("key") || g.contains("NETWORK") || g.contains("FIXTURE"), "{g}");
    }

    #[test]
    fn recorded_http_fixtures() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mt");
        std::env::set_var("OMEGAT_MT_FIXTURE_DIR", &dir);
        let cache = MtCache::default();
        for engine in ["google", "mymemory", "ibmwatson", "apertium", "yandex", "belazar", "mymemory-human"] {
            let recorded = dir.join(engine).join("recorded.json");
            assert!(recorded.exists() || dir.join(format!("{engine}.json")).exists(), "{engine}");
            let r = translate(engine, "Hello", "en", "fr", &cache).unwrap();
            assert!(!r.text.is_empty(), "{engine}");
        }
        std::env::remove_var("OMEGAT_MT_FIXTURE_DIR");
    }

    #[test]
    fn auth_headers_match_java_connectors() {
        let google = auth_headers(
            "google",
            &MtCreds {
                google_key: Some("k".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(google.iter().any(|(k, v)| k == "X-HTTP-Method-Override" && v == "GET"));
        let ibm = auth_headers(
            "ibmwatson",
            &MtCreds {
                ibm_login: Some("apikey".into()),
                ibm_password: Some("secret".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(ibm.iter().any(|(k, v)| k == "X-Watson-Learning-Opt-Out" && v == "true"));
        assert!(ibm.iter().any(|(k, v)| k == "Authorization" && v.starts_with("Basic ")));
        let yandex = auth_headers(
            "yandex",
            &MtCreds {
                yandex_iam: Some("tok".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(yandex[0], ("Authorization".into(), "Bearer tok".into()));
    }

    #[test]
    fn error_body_is_not_a_translation() {
        let raw = r#"{"error":{"message":"API key not valid"}}"#;
        let err = parse_recorded_or_engine("google", raw).unwrap_err();
        assert!(err.contains("API key not valid"));
    }
}
