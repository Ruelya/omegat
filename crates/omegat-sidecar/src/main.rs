//! NDJSON JSON-RPC sidecar. One request per stdin line, one response per stdout line.

use omegat_core::prefs::{default_config_dir, Preferences};
use omegat_core::session::ProjectSession;
use omegat_core::{capabilities, version};
use omegat_ipc::*;
use omegat_plugin::PluginRegistry;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Mutex;

struct App {
    session: Option<ProjectSession>,
    prefs: Preferences,
    plugins: PluginRegistry,
}

impl App {
    fn new() -> Self {
        let prefs = Preferences::load_or_default(&default_config_dir());
        Self {
            session: None,
            prefs,
            plugins: PluginRegistry::new(),
        }
    }

    fn handle(&mut self, req: RpcRequest) -> RpcResponse {
        let id = req.id.clone().unwrap_or(Value::Null);
        match self.dispatch(&req.method, req.params) {
            Ok(result) => RpcResponse::ok(id, result),
            Err((code, msg)) => RpcResponse::err(id, code, msg),
        }
    }

    fn dispatch(&mut self, method: &str, params: Value) -> std::result::Result<Value, (i32, String)> {
        match method {
            "sys.version" => Ok(serde_json::to_value(version()).unwrap()),
            "sys.capabilities" => Ok(serde_json::to_value(capabilities()).unwrap()),
            "sys.plugins" => Ok(serde_json::to_value(self.plugins.list(None)).unwrap()),
            "prefs.get" => Ok(serde_json::to_value(&self.prefs).unwrap()),
            "prefs.set" => {
                if let Ok(p) = serde_json::from_value::<Preferences>(params) {
                    self.prefs = p;
                    let _ = self.prefs.save();
                }
                Ok(serde_json::to_value(&self.prefs).unwrap())
            }
            "project.create" => {
                let p: CreateProjectParams = serde_json::from_value(params).map_err(invalid)?;
                let s = ProjectSession::create(&p, self.prefs.clone()).map_err(core_err)?;
                let dto = s.props.to_dto();
                self.session = Some(s);
                Ok(serde_json::to_value(dto).unwrap())
            }
            "project.open" => {
                let p: OpenProjectParams = serde_json::from_value(params).map_err(invalid)?;
                let s = ProjectSession::open(std::path::Path::new(&p.root), self.prefs.clone())
                    .map_err(core_err)?;
                let dto = s.props.to_dto();
                self.session = Some(s);
                Ok(serde_json::to_value(dto).unwrap())
            }
            "project.close" => {
                if let Some(s) = self.session.as_mut() {
                    let _ = s.save();
                }
                self.session = None;
                Ok(json!({"ok": true}))
            }
            "project.save" => {
                self.session_mut()?.save().map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            "project.compile" => {
                let n = self.session_mut()?.compile(None).map_err(core_err)?;
                Ok(json!({"files": n}))
            }
            "project.props" => Ok(serde_json::to_value(self.session()?.props.to_dto()).unwrap()),
            "entry.list" => {
                let s = self.session()?;
                let list: Vec<EntryDto> = s.entries.iter().enumerate().map(|(i, e)| e.to_dto(i)).collect();
                Ok(serde_json::to_value(list).unwrap())
            }
            "entry.get" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let s = self.session()?;
                let e = s.entries.get(index).ok_or((error_code::INVALID_PARAMS, "index".into()))?;
                Ok(serde_json::to_value(e.to_dto(index)).unwrap())
            }
            "entry.set" => {
                let p: SetEntryParams = serde_json::from_value(params).map_err(invalid)?;
                let e = self.session_mut()?.set_entry(&p).map_err(core_err)?;
                Ok(serde_json::to_value(e).unwrap())
            }
            "matches.query" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                Ok(serde_json::to_value(self.session()?.matches_for(index)).unwrap())
            }
            "glossary.query" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                Ok(serde_json::to_value(self.session()?.glossary_for(index)).unwrap())
            }
            "glossary.add" => {
                let s = self.session_mut()?;
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let comment = params.get("comment").and_then(|v| v.as_str()).unwrap_or("");
                omegat_core::glossary::append_entry(&s.props.glossary_file, source, target, comment)
                    .map_err(|e| (error_code::IO, e.to_string()))?;
                s.glossary = omegat_core::glossary::load_glossary(&s.props.glossary_file);
                Ok(json!({"ok": true}))
            }
            "search.run" => {
                let p: SearchParams = serde_json::from_value(params).map_err(invalid)?;
                Ok(serde_json::to_value(self.session()?.search(&p)).unwrap())
            }
            "stats.get" => Ok(serde_json::to_value(self.session()?.stats()).unwrap()),
            "issues.list" => Ok(serde_json::to_value(self.session()?.issues()).unwrap()),
            "filters.list" => {
                let list: Vec<FilterInfoDto> = omegat_filters::FilterRegistry::new()
                    .info()
                    .into_iter()
                    .map(|f| FilterInfoDto {
                        id: f.id.into(),
                        name: f.name.into(),
                        masks: f.masks.iter().map(|s| (*s).to_string()).collect(),
                        phase: f.phase,
                    })
                    .collect();
                Ok(serde_json::to_value(list).unwrap())
            }
            "mt.query" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let engine = params.get("engine").and_then(|v| v.as_str()).unwrap_or("mock");
                let r = self.session()?.mt(index, engine).map_err(core_err)?;
                Ok(serde_json::to_value(r).unwrap())
            }
            "dict.query" => {
                let word = params.get("word").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.session()?.dict(word)).unwrap())
            }
            "completer.query" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let prefix = params.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.session()?.completer(index, prefix)).unwrap())
            }
            "spell.learn" => {
                let word = params.get("word").and_then(|v| v.as_str()).unwrap_or("");
                let s = self.session_mut()?;
                s.spell.learn(word, &s.props.root);
                Ok(json!({"ok": true}))
            }
            "team.sync" => {
                let s = self.session()?;
                let r = omegat_team::sync(&s.props).map_err(|e| (error_code::INTERNAL_ERROR, e.to_string()))?;
                Ok(json!({"action": r.action, "message": r.message}))
            }
            "script.run" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("null");
                let out = omegat_script::run_source(src, &json!({})).map_err(|e| (error_code::INTERNAL_ERROR, e.to_string()))?;
                Ok(json!({"result": out}))
            }
            "align.run" => {
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                self.session()?
                    .align(std::path::Path::new(source), std::path::Path::new(target), std::path::Path::new(dest))
                    .map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            other => Err((error_code::METHOD_NOT_FOUND, format!("unknown method {other}"))),
        }
    }

    fn session(&self) -> std::result::Result<&ProjectSession, (i32, String)> {
        self.session.as_ref().ok_or((error_code::PROJECT_NOT_OPEN, "no project".into()))
    }
    fn session_mut(&mut self) -> std::result::Result<&mut ProjectSession, (i32, String)> {
        self.session.as_mut().ok_or((error_code::PROJECT_NOT_OPEN, "no project".into()))
    }
}

fn invalid(e: serde_json::Error) -> (i32, String) {
    (error_code::INVALID_PARAMS, e.to_string())
}

fn core_err(e: omegat_core::CoreError) -> (i32, String) {
    let code = match e {
        omegat_core::CoreError::OptimisticLock(_) => error_code::OPTIMISTIC_LOCK,
        omegat_core::CoreError::ProjectNotOpen => error_code::PROJECT_NOT_OPEN,
        omegat_core::CoreError::Io(_) => error_code::IO,
        omegat_core::CoreError::Filter(_) => error_code::FILTER,
        omegat_core::CoreError::TagValidation(_) => error_code::TAG_VALIDATION,
        _ => error_code::INTERNAL_ERROR,
    };
    (code, e.to_string())
}

fn main() {
    let _ = env_logger::try_init();
    let app = Mutex::new(App::new());
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse::err(Value::Null, error_code::PARSE_ERROR, e.to_string());
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
        if req.id.is_none() {
            continue;
        }
        let resp = app.lock().unwrap().handle(req);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
        let _ = stdout.flush();
    }
}
