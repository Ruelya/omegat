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

    /// Load every `omegat-plugin.toml` under `dir` and `dlopen` the `entry` cdylib.
    pub fn load_dir(&mut self, dir: &std::path::Path) -> Result<Vec<String>, PluginError> {
        let mut loaded = Vec::new();
        if !dir.exists() {
            return Ok(loaded);
        }
        let rd = std::fs::read_dir(dir).map_err(|e| PluginError::Manifest(e.to_string()))?;
        for ent in rd.flatten() {
            let p = ent.path();
            let manifest_path = if p.is_dir() {
                let toml = p.join("omegat-plugin.toml");
                let json = p.join("omegat-plugin.json");
                if toml.exists() { toml } else { json }
            } else if p.file_name().and_then(|s| s.to_str()) == Some("omegat-plugin.toml") {
                p
            } else {
                continue;
            };
            if !manifest_path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&manifest_path).map_err(|e| PluginError::Manifest(e.to_string()))?;
            let m = Self::parse_toml(&raw)?;
            if m.entry != "builtin" && !m.entry.is_empty() {
                let lib = manifest_path.parent().unwrap_or(dir).join(&m.entry);
                if lib.exists() {
                    // Safety: plugins are trusted local cdylibs listed in the manifest.
                    let dynlib = unsafe { libloading::Library::new(&lib) }
                        .map_err(|e| PluginError::Manifest(e.to_string()))?;
                    type AbiFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;
                    if let Ok(sym) = unsafe { dynlib.get::<AbiFn>(b"omegat_plugin_abi\0") } {
                        let ptr = unsafe { sym() };
                        if !ptr.is_null() {
                            let _ = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy();
                        }
                    }
                    // Keep the library loaded for the process lifetime.
                    std::mem::forget(dynlib);
                    loaded.push(m.id.clone());
                }
            }
            self.register(m)?;
        }
        Ok(loaded)
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

    #[test]
    fn load_dir_registers_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("omegat-plugin.toml"),
            r#"
id = "demo"
name = "Demo"
version = "1.0.0"
plugin_type = "filter"
entry = "builtin"
"#,
        )
        .unwrap();
        let mut reg = PluginRegistry::new();
        let loaded = reg.load_dir(dir.path()).unwrap();
        assert!(reg.list(None).iter().any(|p| p.id == "demo"));
        let _ = loaded;
    }

    #[test]
    fn loads_example_cdylib_when_built() {
        let mut candidates = vec![
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/debug/libomegat_example_plugin.so"),
            std::path::PathBuf::from("target/debug/libomegat_example_plugin.so"),
        ];
        if cfg!(target_os = "macos") {
            candidates.push(std::path::PathBuf::from("target/debug/libomegat_example_plugin.dylib"));
        }
        let Some(lib) = candidates.into_iter().find(|p| p.exists()) else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(lib.file_name().unwrap());
        std::fs::copy(&lib, &dest).unwrap();
        std::fs::write(
            dir.path().join("omegat-plugin.toml"),
            format!(
                "id = \"example\"\nname = \"Example\"\nversion = \"1.0.0\"\nplugin_type = \"filter\"\nentry = \"{}\"\n",
                dest.file_name().unwrap().to_string_lossy()
            ),
        )
        .unwrap();
        let mut reg = PluginRegistry::new();
        let loaded = reg.load_dir(dir.path()).unwrap();
        assert!(loaded.contains(&"example".to_string()));
    }
}
