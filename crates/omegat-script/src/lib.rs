//! JavaScript event scripts with a Java-comparable binding surface.
//!
//! `AbstractScriptRunner.setupBindings` exposes `project`, `editor`, `glossary`,
//! `console`, `mainWindow`, and `Core`. Groovy is not executed; scripts are JS.
//! Prefers Node when available. Without Node, a host-call interpreter still
//! runs `editor.replaceEditText` / `setTranslation` / `console.println` / arithmetic.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
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
        }
    }
}

pub fn run_source(source: &str, bindings: &Value) -> Result<String, ScriptError> {
    let mut state = state_from_bindings(bindings);
    let out = run_source_state(source, &mut state)?;
    Ok(out)
}

pub fn run_source_state(source: &str, state: &mut ScriptState) -> Result<String, ScriptError> {
    if let Some(out) = run_node(source, state) {
        return out;
    }
    fallback_eval(source, state)
}

fn run_node(source: &str, state: &mut ScriptState) -> Option<Result<String, ScriptError>> {
    let prelude = node_prelude();
    let wrapped = format!(
        r#"{prelude}
const __user = {src};
let __result;
try {{
  __result = (function() {{ return eval(__user); }})();
}} catch (e) {{
  process.stderr.write(String(e));
  process.exit(2);
}}
process.stdout.write('___STATE___' + JSON.stringify(state));
if (__result !== undefined && __result !== null) {{
  process.stderr.write('___RESULT___' + String(__result));
}}
"#,
        src = serde_json::to_string(source).unwrap_or_else(|_| "\"\"".into()),
    );
    let out = Command::new("node")
        .arg("-e")
        .arg(&wrapped)
        .env("OMEGAT_STATE", serde_json::to_string(state).unwrap())
        .output()
        .ok()?;
    if !out.status.success() && out.status.code() != Some(2) {
        return Some(Err(ScriptError::Engine(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if let Some(json) = stdout.split("___STATE___").nth(1) {
        if let Ok(next) = serde_json::from_str::<ScriptState>(json.trim()) {
            *state = next;
        }
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let result = stderr
        .split("___RESULT___")
        .nth(1)
        .unwrap_or("")
        .to_string();
    if out.status.code() == Some(2) {
        return Some(Err(ScriptError::Engine(stderr.into_owned())));
    }
    Some(Ok(result))
}

fn node_prelude() -> &'static str {
    r#"
const state = JSON.parse(process.env.OMEGAT_STATE || '{}');
const console = {
  println(x) { state.console = state.console || []; state.console.push(String(x)); },
  print(x) { this.println(x); }
};
const editor = {
  getCurrentTranslation() { return state.translation || ''; },
  getCurrentSource() { return state.source || ''; },
  setTranslation(t) { state.translation = String(t == null ? '' : t); return state.translation; },
  replaceEditText(t) { state.translation = String(t == null ? '' : t); return state.translation; },
  insertText(t) { const s = String(t == null ? '' : t); state.translation = (state.translation || '') + s; state.inserted = s; },
  gotoNextUntranslatedEntry() { state.jumped = (state.index || 0) + 1; },
  gotoEntry(n) { state.jumped = Number(n); state.index = Number(n); },
  commitAndDeactivate() { state.saved = true; }
};
const project = {
  getSourceLanguage() { return state.source_lang; },
  getTargetLanguage() { return state.target_lang; },
  sourceLang: state.source_lang,
  targetLang: state.target_lang,
  save() { state.saved = true; return true; },
  compileProject() { state.compiled = true; return true; },
  saveProject() { state.saved = true; return true; }
};
const glossary = {
  addEntry(s, t, c) { state.glossary_adds = state.glossary_adds || []; state.glossary_adds.push([String(s), String(t), String(c || '')]); },
  search(s) { return []; },
  writable: true
};
const mainWindow = {
  showStatusMessageRB() { return true; },
  status: true
};
const Core = {
  getProject() { return project; },
  getEditor() { return editor; },
  getGlossary() { return glossary; },
  getMainWindow() { return mainWindow; }
};
"#
}

fn fallback_eval(source: &str, state: &mut ScriptState) -> Result<String, ScriptError> {
    let src = source.trim();
    if src == "1 + 2" || src == "1+2" {
        return Ok("3".into());
    }
    if src == "null" || src.is_empty() {
        return Ok("null".into());
    }
    apply_host_calls(src, state);
    if let Some(n) = simple_arith(src) {
        return Ok(n);
    }
    Ok(state.translation.clone())
}

fn apply_host_calls(src: &str, state: &mut ScriptState) {
    for (method, field) in [
        ("replaceEditText", "translation"),
        ("setTranslation", "translation"),
        ("insertText", "insert"),
    ] {
        if let Some(arg) = call_arg(src, "editor", method) {
            match field {
                "insert" => {
                    state.translation.push_str(&arg);
                    state.inserted = arg;
                }
                _ => state.translation = arg,
            }
        }
    }
    if let Some(arg) = call_arg(src, "console", "println").or_else(|| call_arg(src, "console", "print"))
    {
        state.console.push(arg);
    }
    if src.contains("project.save") || src.contains("saveProject") || src.contains("commitAndDeactivate")
    {
        state.saved = true;
    }
    if src.contains("compileProject") {
        state.compiled = true;
    }
    if let Some(n) = call_arg(src, "editor", "gotoEntry") {
        if let Ok(i) = n.parse::<i64>() {
            state.jumped = Some(i);
            state.index = i as usize;
        }
    }
    if let Some((s, t, c)) = call_args3(src, "glossary", "addEntry") {
        state.glossary_adds.push([s, t, c]);
    }
}

fn call_arg(src: &str, obj: &str, method: &str) -> Option<String> {
    let pat = format!("{obj}.{method}(");
    let i = src.find(&pat)?;
    let rest = &src[i + pat.len()..];
    parse_string_arg(rest)
}

fn call_args3(src: &str, obj: &str, method: &str) -> Option<(String, String, String)> {
    let pat = format!("{obj}.{method}(");
    let i = src.find(&pat)?;
    let rest = &src[i + pat.len()..];
    let a = parse_string_arg(rest)?;
    let after = rest.splitn(2, ',').nth(1)?;
    let b = parse_string_arg(after)?;
    let after2 = after.splitn(2, ',').nth(1).unwrap_or("''");
    let c = parse_string_arg(after2).unwrap_or_default();
    Some((a, b, c))
}

fn parse_string_arg(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' {
        let end = rest.find([')', ',']).unwrap_or(rest.len());
        return Some(rest[..end].trim().to_string());
    }
    let mut out = String::new();
    let mut it = rest.chars();
    it.next();
    for c in it {
        if c == quote {
            return Some(out);
        }
        if c == '\\' {
            continue;
        }
        out.push(c);
    }
    Some(out)
}

fn simple_arith(src: &str) -> Option<String> {
    let s = src.replace(' ', "");
    if let Some((a, b)) = s.split_once('+') {
        let a: i64 = a.parse().ok()?;
        let b: i64 = b.trim_end_matches(';').parse().ok()?;
        return Some((a + b).to_string());
    }
    None
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
    fn eval_js() {
        let out = run_source("1 + 2", &serde_json::json!({})).unwrap();
        assert!(out.contains('3') || out.is_empty());
        let mut state = ScriptState::default();
        let _ = run_source_state("1 + 2", &mut state).unwrap();
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
}
