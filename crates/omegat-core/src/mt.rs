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

pub fn translate(
    engine: &str,
    source: &str,
    source_lang: &str,
    target_lang: &str,
    cache: &MtCache,
) -> Result<MtSuggestionDto, String> {
    let key = format!("{engine}:{source_lang}:{target_lang}:{source}");
    if let Some(text) = cache.get(&key) {
        return Ok(MtSuggestionDto { engine: engine.into(), text });
    }
    if engine == "mock" {
        return Err("mock is not a production engine".into());
    }
    let text = dispatch(engine, source, source_lang, target_lang)?;
    cache.put(key, text.clone());
    Ok(MtSuggestionDto { engine: engine.into(), text })
}

fn dispatch(engine: &str, source: &str, sl: &str, tl: &str) -> Result<String, String> {
    if let Ok(dir) = std::env::var("OMEGAT_MT_FIXTURE_DIR") {
        let path = std::path::Path::new(&dir).join(format!("{engine}.json"));
        if path.exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            return parse_engine(engine, &raw);
        }
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
            let key = std::env::var("OMEGAT_GOOGLE_KEY").map_err(|_| "OMEGAT_GOOGLE_KEY")?;
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
    parse_engine(engine, &raw)
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
        let cache = MtCache::default();
        let err = translate("google", "Hi", "en", "fr", &cache).unwrap_err();
        assert!(err.contains("OMEGAT_MT_NETWORK") || err.contains("FIXTURE"));
    }

    #[test]
    fn recorded_http_fixtures() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mt");
        std::env::set_var("OMEGAT_MT_FIXTURE_DIR", &dir);
        let cache = MtCache::default();
        for engine in ["google", "mymemory", "ibmwatson", "apertium", "yandex", "belazar", "mymemory-human"] {
            let r = translate(engine, "Hello", "en", "fr", &cache).unwrap();
            assert!(!r.text.is_empty(), "{engine}");
        }
        std::env::remove_var("OMEGAT_MT_FIXTURE_DIR");
    }
}
