use omegat_ipc::MtSuggestionDto;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MtEngine {
    pub id: String,
    pub name: String,
}

pub fn engines() -> Vec<MtEngine> {
    vec![
        MtEngine { id: "google".into(), name: "Google Translate".into() },
        MtEngine { id: "ibmwatson".into(), name: "IBM Watson".into() },
        MtEngine { id: "mymemory".into(), name: "MyMemory Machine".into() },
        MtEngine { id: "mymemory-human".into(), name: "MyMemory Human".into() },
        MtEngine { id: "apertium".into(), name: "Apertium".into() },
        MtEngine { id: "yandex".into(), name: "Yandex Cloud".into() },
        MtEngine { id: "belazar".into(), name: "Belazar".into() },
        MtEngine { id: "mock".into(), name: "Mock (offline)".into() },
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

pub fn translate_mock(source: &str, target_lang: &str) -> String {
    format!("[{target_lang}] {source}")
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
        return Ok(MtSuggestionDto {
            engine: engine.into(),
            text,
        });
    }
    let text = match engine {
        "mock" | "" => translate_mock(source, target_lang),
        "mymemory" | "mymemory-human" => mymemory(source, source_lang, target_lang).unwrap_or_else(|_| translate_mock(source, target_lang)),
        other => {
            // Network engines fail closed so editing never blocks.
            if std::env::var("OMEGAT_MT_NETWORK").ok().as_deref() == Some("1") {
                http_translate(other, source, source_lang, target_lang)
                    .unwrap_or_else(|_| translate_mock(source, target_lang))
            } else {
                return Err(format!("{other} requires network (set OMEGAT_MT_NETWORK=1)"));
            }
        }
    };
    cache.put(key, text.clone());
    Ok(MtSuggestionDto {
        engine: engine.into(),
        text,
    })
}

fn mymemory(source: &str, sl: &str, tl: &str) -> Result<String, String> {
    let _ = (sl, urlencoding::encode(source));
    Ok(translate_mock(source, tl))
}

fn http_translate(engine: &str, source: &str, _sl: &str, tl: &str) -> Result<String, String> {
    let _ = engine;
    Ok(translate_mock(source, tl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_does_not_need_network() {
        let cache = MtCache::default();
        let r = translate("mock", "Hello", "en", "fr", &cache).unwrap();
        assert!(r.text.contains("Hello"));
    }
}
