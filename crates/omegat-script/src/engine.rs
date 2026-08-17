//! Embedded Boa interpreter. This is the only script engine.
//!
//! Node is not required. `fallback_eval("1+2")` is gone: arithmetic, `const`,
//! `function`, and host calls all go through real ECMAScript evaluation.

use crate::{ScriptError, ScriptState};
use boa_engine::{Context, JsValue, Source};

/// Java `AbstractScriptRunner.setupBindings` surface, plus `console` / `res`.
const BINDINGS: &str = r#"
var console = {
  println: function(x) {
    state.console = state.console || [];
    state.console.push(String(x));
  },
  print: function(x) { this.println(x); }
};
var editor = {
  getCurrentTranslation: function() { return state.translation || ''; },
  getCurrentSource: function() { return state.source || ''; },
  setTranslation: function(t) {
    state.translation = String(t == null ? '' : t);
    return state.translation;
  },
  replaceEditText: function(t) {
    state.translation = String(t == null ? '' : t);
    return state.translation;
  },
  insertText: function(t) {
    var s = String(t == null ? '' : t);
    state.translation = (state.translation || '') + s;
    state.inserted = s;
    return state.translation;
  },
  gotoNextUntranslatedEntry: function() {
    state.jumped = (state.index || 0) + 1;
  },
  gotoEntry: function(n) {
    state.jumped = Number(n);
    state.index = Number(n);
  },
  commitAndDeactivate: function() { state.saved = true; }
};
var project = {
  getSourceLanguage: function() { return state.source_lang; },
  getTargetLanguage: function() { return state.target_lang; },
  sourceLang: state.source_lang,
  targetLang: state.target_lang,
  save: function() { state.saved = true; return true; },
  compileProject: function() { state.compiled = true; return true; },
  saveProject: function() { state.saved = true; return true; }
};
var glossary = {
  addEntry: function(s, t, c) {
    state.glossary_adds = state.glossary_adds || [];
    state.glossary_adds.push([String(s), String(t), String(c || '')]);
  },
  search: function() { return []; },
  writable: true
};
var mainWindow = {
  showStatusMessageRB: function() { return true; },
  status: true
};
var Core = {
  getProject: function() { return project; },
  getEditor: function() { return editor; },
  getGlossary: function() { return glossary; },
  getMainWindow: function() { return mainWindow; }
};
var res = {};
"#;

pub fn eval(source: &str, state: &mut ScriptState) -> Result<String, ScriptError> {
    let mut context = Context::default();
    let state_json =
        serde_json::to_string(state).map_err(|e| ScriptError::Engine(e.to_string()))?;
    let inject = format!(
        "var state = JSON.parse({});\n{BINDINGS}",
        serde_json::to_string(&state_json).map_err(|e| ScriptError::Engine(e.to_string()))?
    );
    context
        .eval(Source::from_bytes(inject.as_bytes()))
        .map_err(|e| ScriptError::Engine(e.to_string()))?;
    let result = context
        .eval(Source::from_bytes(source.as_bytes()))
        .map_err(|e| ScriptError::Engine(e.to_string()))?;
    let dumped = context
        .eval(Source::from_bytes(b"JSON.stringify(state)"))
        .map_err(|e| ScriptError::Engine(e.to_string()))?;
    let dumped_s = js_to_std_string(&dumped, &mut context)?;
    *state = serde_json::from_str(&dumped_s).map_err(|e| ScriptError::Engine(e.to_string()))?;
    js_result_string(&result, &mut context)
}

fn js_to_std_string(value: &JsValue, context: &mut Context) -> Result<String, ScriptError> {
    if let Some(s) = value.as_string() {
        return Ok(s.to_std_string_escaped());
    }
    value
        .to_string(context)
        .map(|s| s.to_std_string_escaped())
        .map_err(|e| ScriptError::Engine(e.to_string()))
}

fn js_result_string(value: &JsValue, context: &mut Context) -> Result<String, ScriptError> {
    if value.is_undefined() || value.is_null() {
        return Ok(String::new());
    }
    if let Some(n) = value.as_number() {
        if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
            return Ok(format!("{}", n as i64));
        }
        return Ok(n.to_string());
    }
    if let Some(b) = value.as_boolean() {
        return Ok(b.to_string());
    }
    js_to_std_string(value, context)
}
