//! In-process plugin registry. P0 defines the manifest and type map.
//! External cdylib ABI is documented in `docs/rewrite/PLUGIN_ABI.md` and frozen in P9.

use omegat_ipc::PluginManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("unknown plugin type: {0}")]
    UnknownType(String),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("invalid manifest: {0}")]
    Manifest(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Filter,
    Tokenizer,
    Marker,
    Mt,
    Glossary,
    Dictionary,
    Theme,
    Repository,
    Spell,
    Language,
    Misc,
}

impl PluginType {
    pub fn parse(s: &str) -> Result<Self, PluginError> {
        match s.to_ascii_lowercase().as_str() {
            "filter" => Ok(Self::Filter),
            "tokenizer" => Ok(Self::Tokenizer),
            "marker" => Ok(Self::Marker),
            "mt" | "machinetranslator" => Ok(Self::Mt),
            "glossary" => Ok(Self::Glossary),
            "dictionary" => Ok(Self::Dictionary),
            "theme" => Ok(Self::Theme),
            "repository" => Ok(Self::Repository),
            "spell" | "spellcheck" => Ok(Self::Spell),
            "language" => Ok(Self::Language),
            "misc" | "miscellaneous" => Ok(Self::Misc),
            other => Err(PluginError::UnknownType(other.into())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToml {
    pub plugin: PluginManifest,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    by_type: HashMap<PluginType, Vec<PluginManifest>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_builtin();
        reg
    }

    pub fn register(&mut self, manifest: PluginManifest) -> Result<(), PluginError> {
        let ty = PluginType::parse(&manifest.plugin_type)?;
        self.by_type.entry(ty).or_default().push(manifest);
        Ok(())
    }

    pub fn list(&self, ty: Option<PluginType>) -> Vec<PluginManifest> {
        match ty {
            Some(t) => self.by_type.get(&t).cloned().unwrap_or_default(),
            None => self.by_type.values().flatten().cloned().collect(),
        }
    }

    pub fn parse_toml(src: &str) -> Result<PluginManifest, PluginError> {
        if let Ok(parsed) = serde_json::from_str::<PluginToml>(src) {
            PluginType::parse(&parsed.plugin.plugin_type)?;
            return Ok(parsed.plugin);
        }
        let mut id = String::new();
        let mut name = String::new();
        let mut version = String::new();
        let mut plugin_type = String::new();
        let mut entry = String::new();
        for line in src.lines() {
            let line = line.trim();
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "id" => id = v,
                    "name" => name = v,
                    "version" => version = v,
                    "plugin_type" => plugin_type = v,
                    "entry" => entry = v,
                    _ => {}
                }
            }
        }
        if id.is_empty() {
            return Err(PluginError::Manifest("missing id".into()));
        }
        PluginType::parse(&plugin_type)?;
        Ok(PluginManifest {
            id,
            name,
            version,
            plugin_type,
            entry,
        })
    }

    fn register_builtin(&mut self) {
        let builtins = [
            ("core-filters", "Built-in file filters", "filter"),
            ("core-tokenizer", "Unicode / language tokenizers", "tokenizer"),
            ("core-mt", "Machine translation connectors", "mt"),
            ("core-spell", "Spell checker backends", "spell"),
            ("core-dictionary", "StarDict / Lingvo DSL", "dictionary"),
            ("core-theme", "Ink / stone themes", "theme"),
            ("core-repository", "Git / SVN / HTTP / file", "repository"),
            ("core-script", "JavaScript event scripts", "misc"),
        ];
        for (id, name, ty) in builtins {
            let _ = self.register(PluginManifest {
                id: id.into(),
                name: name.into(),
                version: omegat_ipc::APP_VERSION.into(),
                plugin_type: ty.into(),
                entry: "builtin".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_has_filter_type() {
        let reg = PluginRegistry::new();
        assert!(!reg.list(Some(PluginType::Filter)).is_empty());
    }

    #[test]
    fn parse_manifest_toml() {
        let src = r#"
[plugin]
id = "demo"
name = "Demo"
version = "1.0.0"
plugin_type = "filter"
entry = "libdemo.so"
"#;
        let m = PluginRegistry::parse_toml(src).unwrap();
        assert_eq!(m.id, "demo");
    }
}
