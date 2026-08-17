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
        let mut plugins = PluginRegistry::new();
        let _ = plugins.load_dir(&prefs.config_dir.join("plugins"));
        let _ = plugins.load_dir(std::path::Path::new("plugins"));
        Self {
            session: None,
            prefs,
            plugins,
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
                let file = params.get("file").and_then(|v| v.as_str());
                let n = self.session_mut()?.compile(file).map_err(core_err)?;
                Ok(json!({"files": n}))
            }
            "project.reload" => {
                self.session_mut()?.reload().map_err(core_err)?;
                let list: Vec<EntryDto> = self
                    .session()?
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| e.to_dto(i))
                    .collect();
                Ok(json!({"ok": true, "entries": list.len(), "props": self.session()?.props.to_dto()}))
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
            "search.replace" => {
                let p: SearchParams = serde_json::from_value(params).map_err(invalid)?;
                let n = self.session_mut()?.search_replace(&p);
                Ok(json!({"replaced": n}))
            }
            "spell.ignore" => {
                let word = params.get("word").and_then(|v| v.as_str()).unwrap_or("");
                let s = self.session_mut()?;
                s.spell.ignore(word, &s.props.root);
                Ok(json!({"ok": true}))
            }
            "tmx.export" => {
                let level = params.get("level").and_then(|v| v.as_str()).unwrap_or("omegat");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                let s = self.session()?;
                let xml = s.tmx.to_xml_level(&s.props.source_lang, &s.props.target_lang, level);
                if !dest.is_empty() {
                    std::fs::write(dest, &xml).map_err(|e| (error_code::IO, e.to_string()))?;
                }
                Ok(json!({"xml": xml, "level": level}))
            }
            "languagetool.check" => {
                let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let url = self.prefs.extra.get("languagetool_url").cloned();
                let lang = self.session().map(|s| s.props.target_lang.clone()).unwrap_or_else(|_| "en".into());
                Ok(serde_json::to_value(omegat_core::languagetool::check(url.as_deref(), text, &lang, 0, "")).unwrap())
            }
            "finder.run" => {
                let xml = params.get("xml").and_then(|v| v.as_str()).or_else(|| self.prefs.extra.get("finder_xml").map(|s| s.as_str())).unwrap_or("");
                let sel = params.get("selection").and_then(|v| v.as_str()).unwrap_or("");
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or(sel);
                let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let items = omegat_core::finder::parse_finder_xml(xml);
                let mut urls = Vec::new();
                let mut commands = Vec::new();
                for i in &items {
                    if let Some(exp) = omegat_core::finder::expand(i, sel, source, target) {
                        if i.command.is_some() {
                            commands.push(exp);
                        } else {
                            urls.push(exp);
                        }
                    }
                }
                Ok(json!({"urls": urls, "commands": commands, "items": items.len()}))
            }
            "team.conflicts" => {
                let s = self.session()?;
                Ok(json!({"conflicts": omegat_team::list_conflicts(&s.props)}))
            }
            "team.resolve" => {
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let side = params.get("side").and_then(|v| v.as_str()).unwrap_or("ours");
                let translation = params.get("translation").and_then(|v| v.as_str());
                let left = omegat_team::resolve(&self.session()?.props, source, side, translation)
                    .map_err(|e| (error_code::TEAM_CONFLICT, e.to_string()))?;
                Ok(json!({"conflicts": left}))
            }
            "wiki.import" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = &self.session()?.props.source_dir;
                let n = omegat_core::wiki::import_wiki(std::path::Path::new(src), dest).map_err(core_err)?;
                Ok(json!({"files": n}))
            }
            "med.open" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                omegat_core::wiki::open_med(std::path::Path::new(src), std::path::Path::new(dest)).map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            "project.convert" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                let sl = params.get("source_lang").and_then(|v| v.as_str()).unwrap_or("en");
                let tl = params.get("target_lang").and_then(|v| v.as_str()).unwrap_or("fr");
                omegat_core::wiki::convert_project(std::path::Path::new(src), std::path::Path::new(dest), sl, tl).map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            "aligner.configure" => {
                Ok(json!({
                    "modes":["heapwise","parsewise","id"],
                    "algos":["viterbi","forward-backward"],
                    "counters":["char","word"],
                    "calculators":["normal","poisson"]
                }))
            }
            "stats.get" => Ok(serde_json::to_value(self.session()?.stats()).unwrap()),
            "issues.list" => Ok(serde_json::to_value(self.session()?.issues()).unwrap()),
            "filters.options" => {
                let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let reg = omegat_filters::FilterRegistry::new();
                let Some(f) = reg.by_id(id) else {
                    return Err((error_code::INVALID_PARAMS, format!("unknown filter {id}")));
                };
                Ok(json!({
                    "id": f.id(),
                    "name": f.name(),
                    "masks": f.default_masks(),
                    "phase": f.phase(),
                    "options": {
                        "remove_tags": self.prefs.extra.get("remove_tags").cloned().unwrap_or_else(|| "false".into()),
                        "preserve_spaces": self.prefs.extra.get(&format!("filter.{id}.preserve_spaces")).cloned().unwrap_or_else(|| "true".into()),
                        "file_context": self.prefs.extra.get(&format!("filter.{id}.file_context")).cloned().unwrap_or_default(),
                    }
                }))
            }
            "script.slots" => {
                let root = params.get("root").and_then(|v| v.as_str()).unwrap_or("scripts");
                Ok(json!({ "slots": omegat_script::list_slots(std::path::Path::new(root)) }))
            }
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
                let engine = params.get("engine").and_then(|v| v.as_str()).unwrap_or("mymemory");
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
                let draft = params.get("text").and_then(|v| v.as_str());
                Ok(serde_json::to_value(self.session()?.completer(index, prefix, draft)).unwrap())
            }
            "spell.install" => {
                let lang = params.get("lang").and_then(|v| v.as_str()).unwrap_or("en");
                let dest = self.prefs.config_dir.join("spell").join("hunspell");
                let ok = omegat_core::spell::ensure_lang(lang, &dest);
                Ok(json!({"ok": ok, "lang": lang, "dest": dest.display().to_string()}))
            }
            "spell.learn" => {
                let word = params.get("word").and_then(|v| v.as_str()).unwrap_or("");
                let s = self.session_mut()?;
                s.spell.learn(word, &s.props.root);
                Ok(json!({"ok": true}))
            }
            "team.sync" => {
                let s = self.session()?;
                match omegat_team::sync(&s.props) {
                    Ok(r) => Ok(json!({"action": r.action, "message": r.message, "conflicts": r.conflicts})),
                    Err(omegat_team::TeamError::Conflict(msg)) => {
                        Err((error_code::TEAM_CONFLICT, msg))
                    }
                    Err(e) => Err((error_code::INTERNAL_ERROR, e.to_string())),
                }
            }
            "team.commit" => {
                let which = params.get("which").and_then(|v| v.as_str()).unwrap_or("target");
                let r = omegat_team::commit_project_files(&self.session()?.props, which)
                    .map_err(|e| (error_code::INTERNAL_ERROR, e.to_string()))?;
                Ok(json!({"action": r.action, "message": r.message}))
            }
            "script.run" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("null");
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let mut state = if let Ok(s) = self.session() {
                    let e = s.entries.get(index);
                    omegat_script::ScriptState {
                        source: e.map(|e| e.source.clone()).unwrap_or_default(),
                        translation: e.map(|e| e.translation.clone()).unwrap_or_default(),
                        note: e.map(|e| e.note.clone()).unwrap_or_default(),
                        index,
                        revision: e.map(|e| e.revision).unwrap_or(1),
                        source_lang: s.props.source_lang.clone(),
                        target_lang: s.props.target_lang.clone(),
                        ..omegat_script::ScriptState::default()
                    }
                } else {
                    omegat_script::ScriptState::default()
                };
                let out = omegat_script::run_source_state(src, &mut state)
                    .map_err(|e| (error_code::INTERNAL_ERROR, e.to_string()))?;
                if let Ok(s) = self.session_mut() {
                    if let Some(e) = s.entries.get(index) {
                        if state.translation != e.translation {
                            let _ = s.set_entry(&SetEntryParams {
                                index,
                                translation: state.translation.clone(),
                                note: Some(state.note.clone()),
                                revision: e.revision,
                                default_translation: true,
                            });
                        }
                    }
                    if state.saved {
                        let _ = s.save();
                    }
                    if state.compiled {
                        let _ = s.compile(None);
                    }
                    for [src, tgt, cmt] in &state.glossary_adds {
                        let _ = omegat_core::glossary::append_entry(&s.props.glossary_file, src, tgt, cmt);
                    }
                    s.glossary = omegat_core::glossary::load_glossary(&s.props.glossary_file);
                }
                Ok(json!({
                    "result": out,
                    "translation": state.translation,
                    "saved": state.saved,
                    "compiled": state.compiled,
                    "console": state.console,
                    "jumped": state.jumped
                }))
            }
            "align.run" => {
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("parsewise");
                let algo = params.get("algo").and_then(|v| v.as_str()).unwrap_or("viterbi");
                let counter = params.get("counter").and_then(|v| v.as_str()).unwrap_or("word");
                let calculator = params.get("calculator").and_then(|v| v.as_str()).unwrap_or("normal");
                let cfg = omegat_core::align::AlignConfig {
                    mode: match mode {
                        "heapwise" => omegat_core::align::AlignMode::Heapwise,
                        "id" => omegat_core::align::AlignMode::Id,
                        _ => omegat_core::align::AlignMode::Parsewise,
                    },
                    algo: if algo == "forward-backward" {
                        omegat_core::align::AlignAlgo::ForwardBackward
                    } else {
                        omegat_core::align::AlignAlgo::Viterbi
                    },
                    counter: if counter == "char" {
                        omegat_core::align::Counter::Char
                    } else {
                        omegat_core::align::Counter::Word
                    },
                    calculator: if calculator == "poisson" {
                        omegat_core::align::CalculatorType::Poisson
                    } else {
                        omegat_core::align::CalculatorType::Normal
                    },
                    segment: params.get("segment").and_then(|v| v.as_bool()).unwrap_or(true),
                };
                let sl = params
                    .get("source_lang")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| self.session().ok().map(|s| s.props.source_lang.clone()))
                    .unwrap_or_else(|| "en".into());
                let tl = params
                    .get("target_lang")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| self.session().ok().map(|s| s.props.target_lang.clone()))
                    .unwrap_or_else(|| "fr".into());
                let tmx = omegat_core::align::align_files_cfg(
                    std::path::Path::new(source),
                    std::path::Path::new(target),
                    &sl,
                    &tl,
                    &cfg,
                )
                .map_err(core_err)?;
                if !dest.is_empty() {
                    omegat_core::align::write_aligned_tmx(&tmx, std::path::Path::new(dest), &sl, &tl)
                        .map_err(core_err)?;
                }
                let pairs: Vec<_> = tmx
                    .entries
                    .iter()
                    .map(|e| json!({"source": e.source, "target": e.translation}))
                    .collect();
                Ok(json!({"ok": true, "pairs": pairs, "count": pairs.len()}))
            }
            "align.edit" => {
                let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("merge");
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let raw = params.get("pairs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let pairs: Vec<(String, String)> = raw
                    .iter()
                    .map(|v| {
                        (
                            v.get("source").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                            v.get("target").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                        )
                    })
                    .collect();
                let next = omegat_core::align::edit_pairs(&pairs, action, index);
                Ok(json!({
                    "pairs": next.iter().map(|(s,t)| json!({"source": s, "target": t})).collect::<Vec<_>>()
                }))
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
