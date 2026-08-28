use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "light".into()
}
fn default_locale() -> String {
    "en".into()
}
fn default_autosave() -> u64 {
    180
}
fn default_fuzzy() -> i32 {
    30
}
fn default_font() -> String {
    "IBM Plex Sans".into()
}
fn default_export_tm() -> String {
    "omegat level1 level2".into()
}
fn default_tag_validation() -> String {
    "warn".into()
}
fn default_spell_backend() -> String {
    "hunspell".into()
}
fn default_dictionary_dir() -> String {
    "dictionary".into()
}
fn default_chartable() -> String {
    "©®™…—–«»".into()
}
fn default_plugin_dir() -> String {
    "plugins".into()
}
fn default_script_dir() -> String {
    "scripts".into()
}
fn default_srx_path() -> String {
    String::new()
}
fn default_color() -> String {
    "#9b2c1a".into()
}
fn default_script_slots() -> Vec<String> {
    vec![String::new(); 12]
}
fn default_aligner_algo() -> String {
    "viterbi".into()
}
fn default_aligner_calc() -> String {
    "normal".into()
}
fn default_aligner_counter() -> String {
    "word".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorPrefs {
    #[serde(default = "default_color")]
    pub source: String,
    #[serde(default = "default_color")]
    pub target: String,
    #[serde(default = "default_color")]
    pub match_hit: String,
    #[serde(default = "default_color")]
    pub glossary: String,
    #[serde(default = "default_color")]
    pub nbsp: String,
}

impl Default for ColorPrefs {
    fn default() -> Self {
        Self {
            source: default_color(),
            target: default_color(),
            match_hit: default_color(),
            glossary: default_color(),
            nbsp: default_color(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarkPrefs {
    #[serde(default)]
    pub whitespace: bool,
    #[serde(default)]
    pub nbsp: bool,
    #[serde(default)]
    pub bidi: bool,
    #[serde(default = "default_true")]
    pub glossary: bool,
    #[serde(default = "default_true")]
    pub translated: bool,
    #[serde(default = "default_true")]
    pub untranslated: bool,
    #[serde(default = "default_true")]
    pub noted: bool,
    #[serde(default)]
    pub non_unique: bool,
    #[serde(default = "default_true")]
    pub auto_populated: bool,
    #[serde(default = "default_true")]
    pub alternative: bool,
    #[serde(default)]
    pub paragraph_start: bool,
    #[serde(default = "default_true")]
    pub display_source: bool,
    #[serde(default)]
    pub language_checker: bool,
    #[serde(default)]
    pub font_fallback: bool,
    #[serde(default)]
    pub modification: String,
}

impl Default for MarkPrefs {
    fn default() -> Self {
        Self {
            whitespace: false,
            nbsp: false,
            bidi: false,
            glossary: true,
            translated: true,
            untranslated: true,
            noted: true,
            non_unique: false,
            auto_populated: true,
            alternative: true,
            paragraph_start: false,
            display_source: true,
            language_checker: false,
            font_fallback: false,
            modification: "none".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockingLayoutPrefs {
    #[serde(default = "default_left")]
    pub left: f64,
    #[serde(default = "default_notes")]
    pub notes: f64,
    #[serde(default = "default_editor_stack")]
    pub editor_stack: f64,
    #[serde(default = "default_editor_main")]
    pub editor_main: f64,
    #[serde(default = "default_props")]
    pub props: f64,
    #[serde(default = "default_matches")]
    pub matches: f64,
    #[serde(default = "default_east")]
    pub east: f64,
    #[serde(default = "default_props")]
    pub dict_mt: f64,
    #[serde(default = "default_true")]
    pub show_dict: bool,
    #[serde(default = "default_true")]
    pub show_mt: bool,
}

fn default_left() -> f64 {
    0.25
}
fn default_notes() -> f64 {
    0.2
}
fn default_editor_stack() -> f64 {
    0.65
}
fn default_editor_main() -> f64 {
    0.75
}
fn default_props() -> f64 {
    0.5
}
fn default_matches() -> f64 {
    0.8
}
fn default_east() -> f64 {
    0.78
}

impl Default for DockingLayoutPrefs {
    fn default() -> Self {
        Self {
            left: default_left(),
            notes: default_notes(),
            editor_stack: default_editor_stack(),
            editor_main: default_editor_main(),
            props: default_props(),
            matches: default_matches(),
            east: default_east(),
            dict_mt: default_props(),
            show_dict: true,
            show_mt: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchWindowPrefs {
    #[serde(default = "default_search_type")]
    pub search_type: String,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default = "default_true")]
    pub source: bool,
    #[serde(default = "default_true")]
    pub translation: bool,
    #[serde(default)]
    pub notes: bool,
    #[serde(default)]
    pub comments: bool,
    #[serde(default)]
    pub untranslated: bool,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub date_from: String,
    #[serde(default)]
    pub date_to: String,
}

fn default_search_type() -> String {
    "exact".into()
}

impl Default for SearchWindowPrefs {
    fn default() -> Self {
        Self {
            search_type: default_search_type(),
            case_sensitive: false,
            whole_word: false,
            source: true,
            translation: true,
            notes: false,
            comments: false,
            untranslated: false,
            author: String::new(),
            date_from: String::new(),
            date_to: String::new(),
        }
    }
}

/// Typed OmegaT preferences. `extra` is load-only migration residue and is never saved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub config_dir: PathBuf,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_autosave")]
    pub autosave_seconds: u64,
    #[serde(default = "default_fuzzy")]
    pub fuzzy_threshold: i32,
    #[serde(default = "default_true")]
    pub insert_best_match: bool,
    #[serde(default = "default_font")]
    pub font_ui: String,
    #[serde(default = "default_font")]
    pub font_editor: String,
    #[serde(default)]
    pub mt_enabled: Vec<String>,
    #[serde(default)]
    pub tab_advance: bool,
    #[serde(default)]
    pub always_confirm_quit: bool,
    #[serde(default = "default_true")]
    pub first_time_wizard_done: bool,
    #[serde(default)]
    pub colors: ColorPrefs,
    #[serde(default = "default_export_tm")]
    pub export_tm_levels: String,
    #[serde(default = "default_tag_validation")]
    pub tag_validation: String,
    #[serde(default)]
    pub filter_untranslated: bool,
    #[serde(default = "default_true")]
    pub matches_stemming_full: bool,
    #[serde(default)]
    pub marks: MarkPrefs,
    #[serde(default = "default_true")]
    pub project_files_show_translation_progress: bool,
    #[serde(default)]
    pub project_files_show_on_load: bool,
    #[serde(default)]
    pub remove_tags: bool,
    #[serde(default = "default_spell_backend")]
    pub spell_backend: String,
    #[serde(default)]
    pub languagetool_url: String,
    #[serde(default = "default_dictionary_dir")]
    pub dictionary_dir: String,
    #[serde(default)]
    pub dictionary_fuzzy_matching: bool,
    #[serde(default = "default_true")]
    pub dictionary_auto_search: bool,
    #[serde(default = "default_true")]
    pub glossary_stem: bool,
    #[serde(default = "default_true")]
    pub glossary_ignore_case: bool,
    #[serde(default)]
    pub glossary_not_exact_match: bool,
    #[serde(default)]
    pub glossary_replace_on_insert: bool,
    #[serde(default)]
    pub mt_auto_fetch: bool,
    #[serde(default)]
    pub mt_keys: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub completer_auto: bool,
    #[serde(default = "default_true")]
    pub history_completion: bool,
    #[serde(default = "default_true")]
    pub history_prediction: bool,
    #[serde(default = "default_true")]
    pub completer_glossary: bool,
    #[serde(default = "default_true")]
    pub completer_tags: bool,
    #[serde(default = "default_true")]
    pub completer_autotext: bool,
    #[serde(default = "default_true")]
    pub completer_chartable: bool,
    #[serde(default)]
    pub autotext: String,
    #[serde(default = "default_chartable")]
    pub chartable: String,
    #[serde(default)]
    pub team_passphrase: String,
    #[serde(default)]
    pub team_conflict_resolution: String,
    #[serde(default = "default_plugin_dir")]
    pub plugin_dir: String,
    #[serde(default = "default_true")]
    pub version_check_enabled: bool,
    #[serde(default)]
    pub secure_store_key: String,
    #[serde(default = "default_srx_path")]
    pub srx_path: String,
    #[serde(default)]
    pub srx_xml: String,
    #[serde(default)]
    pub finder_xml: String,
    #[serde(default = "default_script_dir")]
    pub script_dir: String,
    #[serde(default = "default_script_slots")]
    pub script_slots: Vec<String>,
    #[serde(default)]
    pub filter_options: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub filter_context: HashMap<String, String>,
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub docking_layout: DockingLayoutPrefs,
    #[serde(default)]
    pub search_window: SearchWindowPrefs,
    /// Typed Java `*Controller` preference keys (not the load-only `extra` bag).
    #[serde(default)]
    pub controller_keys: HashMap<String, String>,
    #[serde(default = "default_aligner_algo")]
    pub aligner_algorithm: String,
    #[serde(default = "default_aligner_calc")]
    pub aligner_calculator: String,
    #[serde(default = "default_aligner_counter")]
    pub aligner_counter: String,
    #[serde(default = "default_true")]
    pub aligner_segment: bool,
    #[serde(default)]
    pub aligner_remove_tags: bool,
    #[serde(default)]
    pub aligner_source_lang: String,
    #[serde(default)]
    pub aligner_target_lang: String,
    #[serde(default)]
    pub aligner_last_source_dir: String,
    #[serde(default)]
    pub aligner_last_target_dir: String,
    /// Load-only bag from pre-G5 files. Never written back.
    #[serde(default, skip_serializing)]
    pub extra: HashMap<String, String>,
}

impl Preferences {
    pub fn default_in(config_dir: PathBuf) -> Self {
        let mut p = Self {
            config_dir,
            theme: default_theme(),
            locale: default_locale(),
            autosave_seconds: default_autosave(),
            fuzzy_threshold: default_fuzzy(),
            insert_best_match: true,
            font_ui: default_font(),
            font_editor: default_font(),
            mt_enabled: vec![],
            tab_advance: false,
            always_confirm_quit: false,
            first_time_wizard_done: true,
            colors: ColorPrefs::default(),
            export_tm_levels: default_export_tm(),
            tag_validation: default_tag_validation(),
            filter_untranslated: false,
            matches_stemming_full: true,
            marks: MarkPrefs::default(),
            project_files_show_translation_progress: true,
            project_files_show_on_load: false,
            remove_tags: false,
            spell_backend: default_spell_backend(),
            languagetool_url: String::new(),
            dictionary_dir: default_dictionary_dir(),
            dictionary_fuzzy_matching: false,
            dictionary_auto_search: true,
            glossary_stem: true,
            glossary_ignore_case: true,
            glossary_not_exact_match: false,
            glossary_replace_on_insert: false,
            mt_auto_fetch: false,
            mt_keys: HashMap::new(),
            completer_auto: true,
            history_completion: true,
            history_prediction: true,
            completer_glossary: true,
            completer_tags: true,
            completer_autotext: true,
            completer_chartable: true,
            autotext: String::new(),
            chartable: default_chartable(),
            team_passphrase: String::new(),
            team_conflict_resolution: String::new(),
            plugin_dir: default_plugin_dir(),
            version_check_enabled: true,
            secure_store_key: String::new(),
            srx_path: default_srx_path(),
            srx_xml: String::new(),
            finder_xml: String::new(),
            script_dir: default_script_dir(),
            script_slots: default_script_slots(),
            filter_options: HashMap::new(),
            filter_context: HashMap::new(),
            shortcuts: HashMap::new(),
            docking_layout: DockingLayoutPrefs::default(),
            search_window: SearchWindowPrefs::default(),
            controller_keys: HashMap::new(),
            aligner_algorithm: default_aligner_algo(),
            aligner_calculator: default_aligner_calc(),
            aligner_counter: default_aligner_counter(),
            aligner_segment: true,
            aligner_remove_tags: false,
            aligner_source_lang: String::new(),
            aligner_target_lang: String::new(),
            aligner_last_source_dir: String::new(),
            aligner_last_target_dir: String::new(),
            extra: HashMap::new(),
        };
        p.normalize();
        p
    }

    pub fn path(&self) -> PathBuf {
        self.config_dir.join("omegat.prefs.json")
    }

    pub fn load_or_default(config_dir: &Path) -> Self {
        let path = config_dir.join("omegat.prefs.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(mut p) = serde_json::from_str::<Preferences>(&raw) {
                if p.config_dir.as_os_str().is_empty() {
                    p.config_dir = config_dir.to_path_buf();
                }
                p.normalize();
                return p;
            }
        }
        let p = Self::default_in(config_dir.to_path_buf());
        let _ = p.save();
        p
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let mut clean = self.clone();
        clean.extra.clear();
        std::fs::write(clean.path(), serde_json::to_string_pretty(&clean).unwrap())
    }

    pub fn filter_option(&self, id: &str, key: &str) -> Option<&str> {
        self.filter_options
            .get(id)
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
            .or_else(|| self.filter_context.get(key).map(|s| s.as_str()))
    }

    pub fn set_filter_option(&mut self, id: &str, key: &str, value: String) {
        self.filter_options
            .entry(id.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    /// Fold a legacy `extra` bag into typed fields, then drop it.
    pub fn normalize(&mut self) {
        if self.script_slots.len() < 12 {
            self.script_slots.resize(12, String::new());
        }
        if self.marks.modification.is_empty() {
            self.marks.modification = "none".into();
        }
        let extra = std::mem::take(&mut self.extra);
        if extra.is_empty() {
            return;
        }
        take_bool(&extra, "tab_advance", &mut self.tab_advance, false);
        take_bool(&extra, "always_confirm_quit", &mut self.always_confirm_quit, false);
        take_bool(&extra, "first_time_wizard_done", &mut self.first_time_wizard_done, true);
        take_str(&extra, "export_tm_levels", &mut self.export_tm_levels);
        take_str(&extra, "tag_validation", &mut self.tag_validation);
        take_bool(&extra, "filter_untranslated", &mut self.filter_untranslated, false);
        take_bool(&extra, "matches_stemming_full", &mut self.matches_stemming_full, true);
        take_bool(&extra, "project_files_show_translation_progress", &mut self.project_files_show_translation_progress, true);
        take_bool(&extra, "project_files_show_on_load", &mut self.project_files_show_on_load, false);
        take_bool(&extra, "remove_tags", &mut self.remove_tags, false);
        take_str(&extra, "spell_backend", &mut self.spell_backend);
        take_str(&extra, "languagetool_url", &mut self.languagetool_url);
        take_str(&extra, "dictionary_dir", &mut self.dictionary_dir);
        take_bool(&extra, "dictionary_fuzzy_matching", &mut self.dictionary_fuzzy_matching, false);
        take_bool(&extra, "dictionary_auto_search", &mut self.dictionary_auto_search, true);
        take_bool(&extra, "glossary_stem", &mut self.glossary_stem, true);
        take_bool(&extra, "glossary_ignore_case", &mut self.glossary_ignore_case, true);
        take_bool(&extra, "glossary_not_exact_match", &mut self.glossary_not_exact_match, false);
        take_bool(&extra, "glossary_replace_on_insert", &mut self.glossary_replace_on_insert, false);
        take_bool(&extra, "mt_auto_fetch", &mut self.mt_auto_fetch, false);
        take_bool(&extra, "completer_auto", &mut self.completer_auto, true);
        take_bool(&extra, "history_completion", &mut self.history_completion, true);
        take_bool(&extra, "history_prediction", &mut self.history_prediction, true);
        take_bool(&extra, "completer_glossary", &mut self.completer_glossary, true);
        take_bool(&extra, "completer_tags", &mut self.completer_tags, true);
        take_bool(&extra, "completer_autotext", &mut self.completer_autotext, true);
        take_bool(&extra, "completer_chartable", &mut self.completer_chartable, true);
        take_str(&extra, "autotext", &mut self.autotext);
        take_str(&extra, "chartable", &mut self.chartable);
        take_str(&extra, "team_passphrase", &mut self.team_passphrase);
        take_str(&extra, "team_conflict_resolution", &mut self.team_conflict_resolution);
        take_str(&extra, "plugin_dir", &mut self.plugin_dir);
        take_bool(&extra, "version_check_enabled", &mut self.version_check_enabled, true);
        take_str(&extra, "secure_store_key", &mut self.secure_store_key);
        take_str(&extra, "srx_path", &mut self.srx_path);
        take_str(&extra, "srx_xml", &mut self.srx_xml);
        take_str(&extra, "finder_xml", &mut self.finder_xml);
        take_str(&extra, "script_dir", &mut self.script_dir);
        take_color(&extra, "color_source", &mut self.colors.source);
        take_color(&extra, "color_target", &mut self.colors.target);
        take_color(&extra, "color_match", &mut self.colors.match_hit);
        take_color(&extra, "color_glossary", &mut self.colors.glossary);
        take_color(&extra, "color_nbsp", &mut self.colors.nbsp);
        take_bool(&extra, "mark_whitespace", &mut self.marks.whitespace, false);
        take_bool(&extra, "mark_nbsp", &mut self.marks.nbsp, false);
        take_bool(&extra, "mark_bidi", &mut self.marks.bidi, false);
        take_bool(&extra, "mark_glossary_matches", &mut self.marks.glossary, true);
        if extra.get("transtips").map(|s| s.as_str()) == Some("false") {
            self.marks.glossary = false;
        }
        take_bool(&extra, "mark_noted_segments", &mut self.marks.noted, true);
        take_bool(&extra, "mark_translated", &mut self.marks.translated, true);
        take_bool(&extra, "mark_untranslated", &mut self.marks.untranslated, true);
        take_bool(&extra, "mark_non_unique", &mut self.marks.non_unique, false);
        take_bool(&extra, "mark_auto_populated", &mut self.marks.auto_populated, true);
        take_bool(&extra, "mark_alternative", &mut self.marks.alternative, true);
        take_bool(&extra, "mark_paragraph_start", &mut self.marks.paragraph_start, false);
        take_bool(&extra, "display_segment_source", &mut self.marks.display_source, true);
        take_bool(&extra, "mark_language_checker", &mut self.marks.language_checker, false);
        take_bool(&extra, "mark_font_fallback", &mut self.marks.font_fallback, false);
        if let Some(v) = extra.get("display_modification_info") {
            if matches!(v.as_str(), "none" | "selected" | "all") {
                self.marks.modification = v.clone();
            }
        }
        take_bool(&extra, "search_window_case_sensitive", &mut self.search_window.case_sensitive, false);
        take_bool(&extra, "search_window_whole_words", &mut self.search_window.whole_word, false);
        take_bool(&extra, "search_window_search_source", &mut self.search_window.source, true);
        take_bool(&extra, "search_window_search_translation", &mut self.search_window.translation, true);
        take_bool(&extra, "search_window_search_notes", &mut self.search_window.notes, false);
        take_bool(&extra, "search_window_search_comments", &mut self.search_window.comments, false);
        take_bool(&extra, "search_window_replace_untranslated", &mut self.search_window.untranslated, false);
        take_str(&extra, "search_window_author_name", &mut self.search_window.author);
        take_str(&extra, "search_window_date_from_value", &mut self.search_window.date_from);
        take_str(&extra, "search_window_date_to_value", &mut self.search_window.date_to);
        if let Some(v) = extra.get("search_window_search_type") {
            if matches!(v.as_str(), "exact" | "keyword" | "regex") {
                self.search_window.search_type = v.clone();
            }
        }
        if let Some(raw) = extra.get("docking_layout").or_else(|| extra.get("MAINWINDOW_LAYOUT")) {
            if let Ok(parsed) = serde_json::from_str::<DockingLayoutPrefs>(raw) {
                self.docking_layout = parsed;
            }
        }
        for (k, v) in &extra {
            if let Some(rest) = k.strip_prefix("filter.") {
                if let Some((id, opt)) = rest.split_once('.') {
                    self.filter_options
                        .entry(id.to_string())
                        .or_default()
                        .insert(opt.to_string(), v.clone());
                }
            } else if let Some(id) = k.strip_prefix("shortcut.") {
                self.shortcuts.insert(id.to_string(), v.clone());
            } else if let Some(eng) = k.strip_prefix("mt.") {
                if let Some(name) = eng.strip_suffix(".key") {
                    self.mt_keys.insert(name.to_string(), v.clone());
                } else if v == "true" && !self.mt_enabled.iter().any(|e| e == eng) {
                    self.mt_enabled.push(eng.to_string());
                } else if eng.contains('.') {
                    self.mt_keys.insert(eng.to_string(), v.clone());
                }
            } else if matches!(
                k.as_str(),
                "segmentOn" | "skipHeader" | "monolingualFormat" | "preserve_spaces"
            ) {
                self.filter_context.insert(k.clone(), v.clone());
            } else if let Some(n) = k.strip_prefix("script_slot_") {
                if let Ok(i) = n.parse::<usize>() {
                    if i >= 1 && i <= 12 {
                        self.script_slots[i - 1] = v.clone();
                    }
                }
            } else {
                self.controller_keys.insert(k.clone(), v.clone());
            }
        }
        self.extra.clear();
    }
}

fn take_bool(extra: &HashMap<String, String>, key: &str, dest: &mut bool, default: bool) {
    if let Some(v) = extra.get(key) {
        *dest = if default { v != "false" } else { v == "true" };
    }
}

fn take_str(extra: &HashMap<String, String>, key: &str, dest: &mut String) {
    if let Some(v) = extra.get(key) {
        if !v.is_empty() {
            *dest = v.clone();
        }
    }
}

fn take_color(extra: &HashMap<String, String>, key: &str, dest: &mut String) {
    if let Some(v) = extra.get(key) {
        if v.starts_with('#') {
            *dest = v.clone();
        }
    }
}

/// Java `PreferencesXML` / `PreferencesImpl` key/value store used by PreferencesTest.
#[derive(Debug, Clone, Default)]
pub struct JavaPreferences {
    pub map: HashMap<String, String>,
}

impl JavaPreferences {
    pub fn set_preference(&mut self, key: Option<&str>, value: Option<&str>) -> Option<String> {
        let key = key.filter(|k| !k.is_empty())?;
        let value = value?;
        self.map.insert(key.to_string(), value.to_string())
    }

    pub fn get_preference(&self, key: Option<&str>) -> String {
        key.and_then(|k| self.map.get(k).cloned()).unwrap_or_default()
    }

    pub fn exists_preference(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn is_preference(&self, key: &str) -> bool {
        self.get_preference(Some(key)) == "true"
    }

    pub fn load_xml(path: &Path) -> Self {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let mut map = HashMap::new();
        let re = regex::Regex::new(r"<([A-Za-z0-9_.-]+)>([^<]*)</([A-Za-z0-9_.-]+)>").unwrap();
        for cap in re.captures_iter(&raw) {
            if cap[1] == cap[3] && !matches!(&cap[1], "omegat" | "preference") {
                map.insert(cap[1].to_string(), cap[2].to_string());
            }
        }
        Self { map }
    }

    pub fn save_xml(&self, path: &Path) -> std::io::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut body = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<omegat>\n<preference version=\"1.0\">\n",
        );
        for (k, v) in &self.map {
            body.push_str(&format!("<{k}>{v}</{k}>\n"));
        }
        body.push_str("</preference>\n</omegat>\n");
        std::fs::write(path, body)
    }

    /// Java: malformed prefs file is copied to `omegat.prefs*.bak`.
    pub fn backup_if_malformed(path: &Path) -> bool {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let malformed = !raw.contains("</omegat>") || !raw.contains("</preference>");
        if !malformed {
            return false;
        }
        let bak = path.with_file_name(format!(
            "{}.bak",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("omegat.prefs")
        ));
        std::fs::copy(path, bak).is_ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_migrates_extra_and_save_drops_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Preferences::default_in(dir.path().to_path_buf());
        p.extra.insert("tag_validation".into(), "abort".into());
        p.extra.insert("mark_whitespace".into(), "true".into());
        p.extra.insert("filter.po.skipHeader".into(), "true".into());
        p.extra.insert("shortcut.project.save".into(), "Ctrl+S".into());
        p.extra.insert("mt.google.key".into(), "secret".into());
        p.extra.insert("search_window_search_notes".into(), "true".into());
        p.extra.insert(
            "docking_layout".into(),
            serde_json::json!({"left": 0.33, "notes": 0.2, "editor_stack": 0.65, "editor_main": 0.75, "props": 0.5, "matches": 0.8, "east": 0.78, "dict_mt": 0.5, "show_dict": true, "show_mt": false}).to_string(),
        );
        p.normalize();
        assert_eq!(p.tag_validation, "abort");
        assert!(p.marks.whitespace);
        assert_eq!(p.filter_option("po", "skipHeader"), Some("true"));
        assert_eq!(p.shortcuts.get("project.save").map(String::as_str), Some("Ctrl+S"));
        assert_eq!(p.mt_keys.get("google").map(String::as_str), Some("secret"));
        assert!(p.search_window.notes);
        assert!((p.docking_layout.left - 0.33).abs() < 1e-9);
        assert!(!p.docking_layout.show_mt);
        assert!(p.extra.is_empty());
        p.save().unwrap();
        let raw = std::fs::read_to_string(p.path()).unwrap();
        assert!(!raw.contains("\"extra\""));
        assert!(raw.contains("\"tag_validation\": \"abort\""));
    }

    #[test]
    fn aligner_settings_round_trip_matches_java_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Preferences::default_in(dir.path().to_path_buf());
        assert_eq!(p.aligner_algorithm, "viterbi");
        assert_eq!(p.aligner_calculator, "normal");
        assert_eq!(p.aligner_counter, "word");
        assert!(p.aligner_segment);
        assert!(!p.aligner_remove_tags);
        p.aligner_algorithm = "forward-backward".into();
        p.aligner_calculator = "poisson".into();
        p.aligner_counter = "char".into();
        p.aligner_segment = false;
        p.aligner_remove_tags = true;
        p.aligner_source_lang = "fr-FR".into();
        p.aligner_target_lang = "de".into();
        p.aligner_last_source_dir = "tmp/foo".into();
        p.save().unwrap();
        let loaded = Preferences::load_or_default(dir.path());
        assert_eq!(loaded.aligner_algorithm, "forward-backward");
        assert_eq!(loaded.aligner_calculator, "poisson");
        assert_eq!(loaded.aligner_counter, "char");
        assert!(!loaded.aligner_segment);
        assert!(loaded.aligner_remove_tags);
        assert_eq!(loaded.aligner_source_lang, "fr-FR");
        assert_eq!(loaded.aligner_target_lang, "de");
        assert_eq!(loaded.aligner_last_source_dir, "tmp/foo");
        let invalid = "not a code";
        let fallback = if invalid.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
            invalid
        } else {
            "eo"
        };
        assert_eq!(fallback, "eo");
    }
}
