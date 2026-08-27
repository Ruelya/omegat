use crate::{
    ensure_parent, placeholder, read_to_string, ExtractedSegment, Filter, FilterContext, FilterError,
    ParsedFile, Result,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub struct JsonFilter;

impl Filter for JsonFilter {
    fn id(&self) -> &'static str {
        "json"
    }
    fn name(&self) -> &'static str {
        "JSON"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.json"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let raw = read_to_string(path)?;
        let value: Value = serde_json::from_str(&raw).map_err(|e| FilterError::Parse {
            format: "json".into(),
            message: e.to_string(),
        })?;
        let mut segments = Vec::new();
        walk(&value, "", &mut segments);
        Ok(ParsedFile {
            segments,
            skeleton: Some(raw),
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let mut value: Value = serde_json::from_str(&raw).map_err(|e| FilterError::Parse {
            format: "json".into(),
            message: e.to_string(),
        })?;
        apply(&mut value, "", translations);
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, serde_json::to_string_pretty(&value).unwrap_or(raw))?;
        Ok(())
    }
}

fn walk(value: &Value, path: &str, out: &mut Vec<ExtractedSegment>) {
    match value {
        Value::String(s) if !s.is_empty() => {
            out.push(ExtractedSegment {
                id: if path.is_empty() {
                    out.len().to_string()
                } else {
                    path.to_string()
                },
                source: s.clone(),
                existing_translation: None,
                note: None,
                comment: None,
                path: Some(path.to_string()),
                protected_parts: vec![],
            });
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                walk(v, &format!("{path}[{i}]"), out);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                let next = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                walk(v, &next, out);
            }
        }
        _ => {}
    }
}

fn apply(value: &mut Value, path: &str, translations: &HashMap<String, String>) {
    match value {
        Value::String(s) => {
            if let Some(t) = translations.get(path).or_else(|| translations.get(&placeholder(0))) {
                *s = t.clone();
            } else if let Some(t) = translations.values().find(|_| true) {
                let _ = t;
            }
            if let Some(t) = translations.get(path) {
                *s = t.clone();
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter_mut().enumerate() {
                apply(v, &format!("{path}[{i}]"), translations);
            }
        }
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let next = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                apply(v, &next, translations);
            }
        }
        _ => {}
    }
}
