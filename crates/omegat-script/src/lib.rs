//! JavaScript event scripts with a Java-comparable binding surface.
//!
//! `AbstractScriptRunner.setupBindings` exposes `project`, `editor`, `glossary`,
//! `console`, `mainWindow`, and `Core`. Groovy is not executed; scripts are JS
//! evaluated by the embedded Boa engine. Node is not required.

mod engine;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("script: {0}")]
    Engine(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEvent {
    ApplicationStartup,
    ApplicationShutdown,
    ProjectChanged,
    EntryActivated,
    NewFile,
    NewWord,
}

impl ScriptEvent {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::ApplicationStartup => "application_startup",
            Self::ApplicationShutdown => "application_shutdown",
            Self::ProjectChanged => "project_changed",
            Self::EntryActivated => "entry_activated",
            Self::NewFile => "new_file",
            Self::NewWord => "new_word",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "application_startup" | "APPLICATION_STARTUP" => Some(Self::ApplicationStartup),
            "application_shutdown" | "APPLICATION_SHUTDOWN" => Some(Self::ApplicationShutdown),
            "project_changed" | "PROJECT_CHANGE" => Some(Self::ProjectChanged),
            "entry_activated" | "ENTRY_ACTIVATED" => Some(Self::EntryActivated),
            "new_file" | "NEW_FILE" => Some(Self::NewFile),
            "new_word" | "NEW_WORD" => Some(Self::NewWord),
            _ => None,
        }
    }

    pub fn all() -> [Self; 6] {
        [
            Self::ApplicationStartup,
            Self::ApplicationShutdown,
            Self::ProjectChanged,
            Self::EntryActivated,
            Self::NewFile,
            Self::NewWord,
        ]
    }
}

/// Mutable host state the script bindings read and write (Java IEditor / IProject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptState {
    pub source: String,
    pub translation: String,
    pub note: String,
    pub index: usize,
    pub revision: u64,
    pub source_lang: String,
    pub target_lang: String,
    pub saved: bool,
    pub compiled: bool,
    pub jumped: Option<i64>,
    pub glossary_adds: Vec<[String; 3]>,
    pub console: Vec<String>,
    pub inserted: String,
    #[serde(default)]
    pub activated: bool,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub selected: String,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub completer: Vec<String>,
    #[serde(default)]
    pub case_mode: String,
    #[serde(default)]
    pub history: String,
    #[serde(default)]
    pub popups: bool,
    #[serde(default)]
    pub alternate: bool,
    #[serde(default)]
    pub remarked: bool,
    #[serde(default)]
    pub refreshed: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub undone: bool,
    #[serde(default)]
    pub redone: bool,
    #[serde(default)]
    pub deactivated: bool,
}

impl Default for ScriptState {
    fn default() -> Self {
        Self {
            source: String::new(),
            translation: String::new(),
            note: String::new(),
            index: 0,
            revision: 1,
            source_lang: "en".into(),
            target_lang: "fr".into(),
            saved: false,
            compiled: false,
            jumped: None,
            glossary_adds: vec![],
            console: vec![],
            inserted: String::new(),
            activated: false,
            file: String::new(),
            target_file: String::new(),
            selected: String::new(),
            filter: None,
            settings: serde_json::json!({}),
            completer: vec![],
            case_mode: String::new(),
            history: String::new(),
            popups: false,
            alternate: false,
            remarked: false,
            refreshed: false,
            focused: false,
            undone: false,
            redone: false,
            deactivated: false,
        }
    }
}

pub fn run_source(source: &str, bindings: &Value) -> Result<String, ScriptError> {
    let mut state = state_from_bindings(bindings);
    let out = run_source_state(source, &mut state)?;
    Ok(out)
}

pub fn run_source_state(source: &str, state: &mut ScriptState) -> Result<String, ScriptError> {
    engine::eval(source, state)
}

fn state_from_bindings(bindings: &Value) -> ScriptState {
    let mut s = ScriptState::default();
    if let Some(p) = bindings.get("project") {
        if let Some(v) = p.get("sourceLang").and_then(|v| v.as_str()) {
            s.source_lang = v.into();
        }
        if let Some(v) = p.get("targetLang").and_then(|v| v.as_str()) {
            s.target_lang = v.into();
        }
    }
    if let Some(e) = bindings.get("editor") {
        if let Some(v) = e.get("translation").and_then(|v| v.as_str()) {
            s.translation = v.into();
        }
        if let Some(v) = e.get("source").and_then(|v| v.as_str()) {
            s.source = v.into();
        }
        if let Some(v) = e.get("index").and_then(|v| v.as_u64()) {
            s.index = v as usize;
        }
    }
    if let Some(v) = bindings.get("source").and_then(|v| v.as_str()) {
        s.source = v.into();
    }
    if let Some(v) = bindings.get("translation").and_then(|v| v.as_str()) {
        s.translation = v.into();
    }
    s
}

/// Twelve historic shortcut slots (`scripts/slot01.js` … `slot12.js`).
pub fn list_slots(scripts_root: &Path) -> Vec<String> {
    (1..=12)
        .filter_map(|i| {
            let name = format!("slot{i:02}.js");
            let p = scripts_root.join(&name);
            if p.exists() {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

pub fn run_slot(scripts_root: &Path, slot: u8, bindings: &Value) -> Result<String, ScriptError> {
    let path = scripts_root.join(format!("slot{slot:02}.js"));
    let src = std::fs::read_to_string(path)?;
    run_source(&src, bindings)
}

pub fn default_bindings(event: &str) -> Value {
    serde_json::json!({
        "event": event,
        "project": { "sourceLang": "en", "targetLang": "fr" },
        "editor": { "insert": true, "translation": "", "source": "" },
        "glossary": { "writable": true },
        "console": { "println": true },
        "mainWindow": { "status": true },
        "Core": { "getEditor": true, "getProject": true }
    })
}

pub fn run_event_dir(
    scripts_root: &Path,
    event: ScriptEvent,
    bindings: &Value,
) -> Result<Vec<String>, ScriptError> {
    let dir = scripts_root.join(event.dir_name());
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut logs = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("js"))
        .collect();
    files.sort();
    for f in files {
        let src = std::fs::read_to_string(&f)?;
        logs.push(run_source(&src, bindings)?);
    }
    Ok(logs)
}

pub fn run_event_dir_state(
    scripts_root: &Path,
    event: ScriptEvent,
    state: &mut ScriptState,
) -> Result<Vec<String>, ScriptError> {
    let dir = scripts_root.join(event.dir_name());
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut logs = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("js"))
        .collect();
    files.sort();
    for f in files {
        let src = std::fs::read_to_string(&f)?;
        logs.push(run_source_state(&src, state)?);
    }
    Ok(logs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_js_arithmetic_is_real_engine() {
        let out = run_source("1 + 2", &serde_json::json!({})).unwrap();
        assert_eq!(out, "3");
        let out = run_source("(function () { return 10 + 20; })()", &serde_json::json!({})).unwrap();
        assert_eq!(out, "30");
    }

    #[test]
    fn entry_activated_can_change_translation() {
        let mut state = ScriptState {
            source: "Hi".into(),
            translation: "x".into(),
            ..ScriptState::default()
        };
        run_source_state("editor.replaceEditText('Bonjour')", &mut state).unwrap();
        assert_eq!(state.translation, "Bonjour");
    }

    #[test]
    fn bindings_are_real_js_not_string_scan() {
        let mut state = ScriptState::default();
        run_source_state(
            "const t = 'Bonjour'; editor.replaceEditText(t);",
            &mut state,
        )
        .unwrap();
        assert_eq!(state.translation, "Bonjour");

        let mut state = ScriptState::default();
        run_source_state(
            "function twice(s) { return s + s; } editor.replaceEditText(twice('X'));",
            &mut state,
        )
        .unwrap();
        assert_eq!(state.translation, "XX");
    }

    #[test]
    fn invalid_js_is_an_error() {
        let mut state = ScriptState::default();
        let err = run_source_state("this is not valid javascript !!!", &mut state).unwrap_err();
        assert!(matches!(err, ScriptError::Engine(_)));
    }

    #[test]
    fn bindings_cover_project_glossary_console() {
        let mut state = ScriptState::default();
        run_source_state(
            "project.save(); glossary.addEntry('cat','chat','n'); console.println('ok');",
            &mut state,
        )
        .unwrap();
        assert!(state.saved);
        assert_eq!(
            state.glossary_adds[0],
            ["cat".to_string(), "chat".to_string(), "n".to_string()]
        );
        assert_eq!(state.console, vec!["ok".to_string()]);
    }

    #[test]
    fn slots_and_six_event_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("slot01.js"), "1 + 2").unwrap();
        assert_eq!(list_slots(dir.path()), vec!["slot01.js".to_string()]);
        for ev in ScriptEvent::all() {
            let p = dir.path().join(ev.dir_name());
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("hook.js"), "console.println('e')").unwrap();
        }
        let mut state = ScriptState::default();
        let logs = run_event_dir_state(dir.path(), ScriptEvent::EntryActivated, &mut state).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(state.console, vec!["e".to_string()]);
        assert_eq!(ScriptEvent::all().len(), 6);
    }

    #[test]
    fn boa_editor_exposes_ieditor_method_set() {
        let mut state = ScriptState {
            source: "Hi".into(),
            translation: "x".into(),
            index: 2,
            ..ScriptState::default()
        };
        run_source_state(
            r#"
            editor.activateEntry();
            editor.changeCase('upper');
            editor.commitAndDeactivate();
            editor.commitAndLeave();
            editor.getAutoCompleter();
            editor.getCurrentEntryNumber();
            editor.getCurrentFile();
            editor.getCurrentTargetFile();
            editor.getCurrentTranslation();
            editor.getFilter();
            editor.getSelectedText();
            editor.getSettings();
            editor.getCurrentPositionInEntryTranslationInEditor();
            editor.gotoEntry(3);
            editor.gotoEntryAfterFix(3);
            editor.gotoFile('a.txt');
            editor.gotoHistoryBack();
            editor.gotoHistoryForward();
            editor.insertTag('<x0/>');
            editor.insertText('!');
            editor.insertTextAndMark('?');
            editor.isOrientationAllLtr();
            editor.markActiveEntrySource();
            editor.nextEntry();
            editor.nextEntryWithNote();
            editor.nextTranslatedEntry();
            editor.nextUniqueEntry();
            editor.nextUntranslatedEntry();
            editor.nextXAutoEntry();
            editor.nextXEnforcedEntry();
            editor.prevEntry();
            editor.prevEntryWithNote();
            editor.prevXAutoEntry();
            editor.prevXEnforcedEntry();
            editor.redo();
            editor.refreshView();
            editor.refreshViewAfterFix();
            editor.registerEmptyTranslation();
            editor.registerIdenticalTranslation();
            editor.registerPopupMenuConstructors();
            editor.registerUntranslated();
            editor.remarkOneMarker();
            editor.removeFilter();
            editor.replaceEditText('Bonjour');
            editor.replaceEditTextAndMark('Bonjour');
            editor.requestFocus();
            editor.selectSourceText();
            editor.setAlternateTranslationForCurrentEntry(true);
            editor.setFilter('untranslated');
            editor.undo();
            editor.windowDeactivated();
            "#,
            &mut state,
        )
        .unwrap();
        assert_eq!(state.translation, "Bonjour");
        assert!(state.saved);
        assert!(state.activated);
        assert_eq!(state.file, "a.txt");
    }

    #[test]
    fn source_has_no_string_eval_shim() {
        let src = concat!(include_str!("lib.rs"), include_str!("engine.rs"));
        let shim = ["fn ", "fallback", "_eval"].concat();
        let arith = ["fn ", "simple", "_arith"].concat();
        assert!(!src.contains(&shim));
        assert!(!src.contains(&arith));
        assert!(!src.contains("Command::new(\"node\")"));
    }
}
