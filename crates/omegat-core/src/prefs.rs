use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub config_dir: PathBuf,
    pub theme: String,
    pub locale: String,
    pub autosave_seconds: u64,
    pub fuzzy_threshold: i32,
    pub insert_best_match: bool,
    pub font_ui: String,
    pub font_editor: String,
    pub mt_enabled: Vec<String>,
    pub extra: HashMap<String, String>,
}

impl Preferences {
    pub fn default_in(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            theme: "light".into(),
            locale: "en".into(),
            autosave_seconds: 180,
            fuzzy_threshold: 30,
            insert_best_match: true,
            font_ui: "IBM Plex Sans".into(),
            font_editor: "IBM Plex Sans".into(),
            mt_enabled: vec![],
            extra: HashMap::new(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.config_dir.join("omegat.prefs.json")
    }

    pub fn load_or_default(config_dir: &Path) -> Self {
        let path = config_dir.join("omegat.prefs.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(p) = serde_json::from_str(&raw) {
                return p;
            }
        }
        let p = Self::default_in(config_dir.to_path_buf());
        let _ = p.save();
        p
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::write(self.path(), serde_json::to_string_pretty(self).unwrap())
    }
}

pub fn default_config_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("OMEGAT_CONFIG_DIR") {
        return PathBuf::from(p);
    }
    dirs_config()
}

fn dirs_config() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".omegat");
    }
    PathBuf::from(".omegat")
}
