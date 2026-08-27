//! Seven Java MT connectors, one module each. Offline without a recorded
//! fixture is an error and must not block editing.

pub mod apertium;
pub mod belazar;
pub mod google;
pub mod ibmwatson;
pub mod mymemory;
pub mod mymemory_human;
pub mod yandex;

use crate::cancellation::CancellationToken;
use crate::languagetool::http_exchange_cancellable;
use omegat_ipc::MtSuggestionDto;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MtEngine {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub glossary_supplier: Option<String>,
}

pub fn engines() -> Vec<MtEngine> {
    vec![
        MtEngine { id: google::ID.into(), name: "Google Translate".into(), endpoint: google::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: ibmwatson::ID.into(), name: "IBM Watson".into(), endpoint: ibmwatson::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: mymemory::ID.into(), name: "MyMemory Machine".into(), endpoint: mymemory::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: mymemory_human::ID.into(), name: "MyMemory Human".into(), endpoint: mymemory_human::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: apertium::ID.into(), name: "Apertium".into(), endpoint: apertium::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: yandex::ID.into(), name: "Yandex Cloud".into(), endpoint: yandex::ENDPOINT.into(), glossary_supplier: None },
        MtEngine { id: belazar::ID.into(), name: "Belazar".into(), endpoint: belazar::ENDPOINT.into(), glossary_supplier: None },
    ]
}

/// Java `MachineTranslatorsManager.setGlossaryMap`.
pub fn set_glossary_map(translators: &mut [MtEngine], supplier: Option<&str>) {
    for t in translators {
        t.glossary_supplier = supplier.map(|s| s.to_string());
    }
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
    pub fn from_prefs(prefs: &crate::prefs::Preferences) -> Self {
        let keys = &prefs.mt_keys;
        Self {
            google_key: keys.get("google").cloned().or_else(|| std::env::var("OMEGAT_GOOGLE_KEY").ok()),
            ibm_login: keys.get("ibmwatson.login").cloned().or_else(|| keys.get("ibmwatson").cloned()),
            ibm_password: keys.get("ibmwatson.password").cloned(),
            yandex_iam: keys.get("yandex").cloned().or_else(|| std::env::var("OMEGAT_YANDEX_IAM").ok()),
            mymemory_key: keys.get("mymemory").cloned(),
        }
    }
}

pub fn auth_headers(engine: &str, creds: &MtCreds) -> Result<Vec<(String, String)>, String> {
    match engine {
        "google" => google::auth_headers(!creds.google_key.as_deref().unwrap_or("").is_empty()),
        "ibmwatson" => {
            let login = creds.ibm_login.clone().unwrap_or_else(|| "apikey".into());
            let pass = creds.ibm_password.clone().or_else(|| creds.ibm_login.clone()).unwrap_or_default();
            ibmwatson::auth_headers(&login, &pass)
        }
        "yandex" => yandex::auth_headers(creds.yandex_iam.as_deref().unwrap_or("")),
        "mymemory" | "mymemory-human" | "apertium" | "belazar" => Ok(vec![]),
        other => Err(format!("unknown engine {other}")),
    }
}

pub(crate) fn base64_basic(s: &str) -> String {
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
    translate_with_creds_cancellable(
        engine,
        source,
        source_lang,
        target_lang,
        cache,
        creds,
        &CancellationToken::default(),
    )
}

pub fn translate_with_creds_cancellable(
    engine: &str,
    source: &str,
    source_lang: &str,
    target_lang: &str,
    cache: &MtCache,
    creds: &MtCreds,
    cancellation: &CancellationToken,
) -> Result<MtSuggestionDto, String> {
    if cancellation.is_cancelled() {
        return Err("request cancelled".into());
    }
    let key = format!("{engine}:{source_lang}:{target_lang}:{source}");
    if let Some(text) = cache.get(&key) {
        return Ok(MtSuggestionDto { engine: engine.into(), text });
    }
    if engine == "mock" {
        return Err("mock is not a production engine".into());
    }
    let _ = auth_headers(engine, creds)?;
    let text = dispatch_cancellable(
        engine,
        source,
        source_lang,
        target_lang,
        creds,
        cancellation,
    )?;
    if cancellation.is_cancelled() {
        return Err("request cancelled".into());
    }
    cache.put(key, text.clone());
    Ok(MtSuggestionDto { engine: engine.into(), text })
}

fn dispatch(engine: &str, source: &str, sl: &str, tl: &str, creds: &MtCreds) -> Result<String, String> {
    dispatch_cancellable(
        engine,
        source,
        sl,
        tl,
        creds,
        &CancellationToken::default(),
    )
}

fn dispatch_cancellable(
    engine: &str,
    source: &str,
    sl: &str,
    tl: &str,
    creds: &MtCreds,
    cancellation: &CancellationToken,
) -> Result<String, String> {
    if cancellation.is_cancelled() {
        return Err("request cancelled".into());
    }
    if let Ok(dir) = std::env::var("OMEGAT_MT_FIXTURE_DIR") {
        let recorded = std::path::Path::new(&dir).join(engine).join("recorded.json");
        let legacy = std::path::Path::new(&dir).join(format!("{engine}.json"));
        let path = if recorded.exists() { recorded } else { legacy };
        if path.exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            if cancellation.is_cancelled() {
                return Err("request cancelled".into());
            }
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
                "{}?q={}&langpair={}|{}",
                mymemory::ENDPOINT,
                urlencoding::encode(source),
                urlencoding::encode(sl),
                urlencoding::encode(tl)
            );
            http_exchange_cancellable("GET", &url, None, cancellation)?
        }
        "apertium" => {
            let url = format!(
                "{}?q={}&langpair={}|{}",
                apertium::ENDPOINT,
                urlencoding::encode(source),
                urlencoding::encode(sl),
                urlencoding::encode(tl)
            );
            http_exchange_cancellable("GET", &url, None, cancellation)?
        }
        "google" => {
            let key = creds
                .google_key
                .clone()
                .or_else(|| std::env::var("OMEGAT_GOOGLE_KEY").ok())
                .ok_or("OMEGAT_GOOGLE_KEY")?;
            let url = format!("{}?key={}", google::ENDPOINT, urlencoding::encode(&key));
            let body = format!(
                "{{\"q\":\"{}\",\"source\":\"{}\",\"target\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange_cancellable(
                "POST",
                &url,
                Some(("application/json", &body)),
                cancellation,
            )?
        }
        "ibmwatson" => {
            let url = std::env::var("OMEGAT_IBM_URL").unwrap_or_else(|_| ibmwatson::ENDPOINT.to_string());
            let body = format!(
                "{{\"text\":[\"{}\"],\"source\":\"{}\",\"target\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange_cancellable(
                "POST",
                &url,
                Some(("application/json", &body)),
                cancellation,
            )?
        }
        "yandex" => {
            let url = std::env::var("OMEGAT_YANDEX_URL").unwrap_or_else(|_| yandex::ENDPOINT.to_string());
            let body = format!(
                "{{\"texts\":[\"{}\"],\"sourceLanguageCode\":\"{}\",\"targetLanguageCode\":\"{}\"}}",
                escape_json(source), sl, tl
            );
            http_exchange_cancellable(
                "POST",
                &url,
                Some(("application/json", &body)),
                cancellation,
            )?
        }
        "belazar" => {
            let url = format!(
                "{}?text={}&sl={}&tl={}",
                belazar::ENDPOINT,
                urlencoding::encode(source), sl, tl
            );
            http_exchange_cancellable("GET", &url, None, cancellation)?
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
        "google" => google::parse(&v),
        "ibmwatson" => ibmwatson::parse(&v),
        "mymemory" => mymemory::parse(&v),
        "mymemory-human" => mymemory_human::parse(&v),
        "apertium" => apertium::parse(&v),
        "yandex" => yandex::parse(&v),
        "belazar" => belazar::parse(&v),
        _ => v.get("text").and_then(|x| x.as_str()).map(str::to_string),
    };
    text.map(|s| html_escape::decode_html_entities(&s).into_owned())
        .ok_or_else(|| format!("{engine} response missing translation"))
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_connector_modules() {
        assert_eq!(engines().len(), 7);
        for id in ["google", "ibmwatson", "mymemory", "mymemory-human", "apertium", "yandex", "belazar"] {
            assert!(engines().iter().any(|e| e.id == id), "{id}");
        }
    }

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
            let expected = match engine {
                "google" => "Hola",
                "ibmwatson" | "apertium" | "yandex" => "Bonjour",
                "mymemory" | "mymemory-human" => "Bonjour le monde",
                "belazar" => "Прывітанне",
                _ => "",
            };
            assert_eq!(r.text, expected, "{engine}");
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
