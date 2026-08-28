//! NDJSON JSON-RPC sidecar. One request per stdin line, one response per stdout line.

mod project_watcher;
mod refresh_journal;

use omegat_core::cancellation::CancellationToken;
use omegat_core::prefs::{default_config_dir, Preferences};
use omegat_core::session::ProjectSession;
use omegat_core::{capabilities, version};
use omegat_ipc::*;
use omegat_plugin::PluginRegistry;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

fn mutable_bead_from_json(value: &Value) -> omegat_core::align::MutableBead {
    let lines = |key: &str, fallback: &str| -> Vec<Option<String>> {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .map(|line| line.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![Some(
                    value
                        .get(fallback)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )]
            })
    };
    let source_lines = lines("source_lines", "source");
    let target_lines = lines("target_lines", "target");
    let mut bead = omegat_core::align::MutableBead::from_lines(
        value
            .get("score")
            .and_then(Value::as_f64)
            .unwrap_or(f32::MAX as f64) as f32,
        source_lines,
        target_lines,
    );
    if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
        bead.enabled = enabled;
    }
    bead.status = match value.get("status").and_then(Value::as_str) {
        Some("accepted") => omegat_core::align::BeadStatus::Accepted,
        Some("needs-review") => omegat_core::align::BeadStatus::NeedsReview,
        _ => omegat_core::align::BeadStatus::Default,
    };
    bead
}

fn mutable_bead_json(
    bead: &omegat_core::align::MutableBead,
    source_language: &str,
    target_language: &str,
) -> Value {
    json!({
        "source": bead.source_text(source_language),
        "target": bead.target_text(target_language),
        "source_lines": &bead.source_lines,
        "target_lines": &bead.target_lines,
        "score": bead.score,
        "enabled": bead.enabled,
        "status": bead.status,
    })
}

struct App {
    session: Option<ProjectSession>,
    prefs: Preferences,
    plugins: PluginRegistry,
}

impl App {
    fn new() -> Self {
        let config_dir = default_config_dir();
        let mut prefs = Preferences::load_or_default(&config_dir);
        let scripts = std::env::var_os("OMEGAT_SCRIPTS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(&prefs.script_dir));
        let scripts = if scripts.is_absolute() {
            scripts
        } else {
            config_dir.join(scripts)
        };
        let scripts = omegat_core::cli_params::resolve_scripts_folder(Some(&scripts))
            .unwrap_or_else(|| omegat_core::cli_params::default_user_scripts_dir(&config_dir));
        let _ = std::fs::create_dir_all(&scripts);
        prefs.script_dir = scripts.to_string_lossy().into_owned();
        let mut plugins = PluginRegistry::new();
        if let Ok(executable) = std::env::current_exe() {
            plugins.enable_marker_isolation(executable);
        }
        plugins.load_default_dirs(&prefs.config_dir);
        Self {
            session: None,
            prefs,
            plugins,
        }
    }

    fn handle(&mut self, req: RpcRequest, cancellation: &CancellationToken) -> RpcResponse {
        let id = req.id.clone().unwrap_or(Value::Null);
        let result = self.dispatch(&req.method, req.params, cancellation);
        match result {
            Ok(result) => RpcResponse::ok(id, result),
            Err((code, msg)) => RpcResponse::err(id, code, msg),
        }
    }

    fn handle_external_refresh_transactional(
        &mut self,
        id: Value,
        cancellation: &CancellationToken,
        publish_commit: impl FnOnce(&Value) -> Result<(), String>,
    ) -> RpcResponse {
        let mut committed_result = None;
        let result = self.session_mut().and_then(|session| {
            session
                .refresh_external_cancellable_before_commit(cancellation, |candidate| {
                    let result = external_refresh_result(candidate);
                    publish_commit(&result).map_err(omegat_core::CoreError::InvalidProject)?;
                    committed_result = Some(result);
                    Ok(())
                })
                .map_err(core_err)?;
            committed_result.ok_or((
                error_code::INTERNAL_ERROR,
                "external refresh committed without a product result".into(),
            ))
        });
        match result {
            Ok(result) => RpcResponse::ok(id, result),
            Err((code, message)) => RpcResponse::err(id, code, message),
        }
    }

    fn save_product_transaction(
        &mut self,
        operation: &str,
        params: &Value,
        cancellation: &CancellationToken,
    ) -> std::result::Result<Option<omegat_team::TransactionRendererReceipt>, (i32, String)> {
        let root = self.session()?.props.root.clone();
        let (generation, batch_id) = transaction_scope(params, &root)?;
        let session = self.session_mut()?;
        let checkpoint = session.checkpoint();
        let props = session.props.clone();
        let result = omegat_team::commit_product_transaction_cancellable(
            &props,
            operation,
            cancellation,
            "project.product.snapshot",
            generation,
            batch_id.as_deref(),
            |_| session.save().map_err(core_product_err),
        );
        if let Err(error) = result {
            session.restore_checkpoint(checkpoint);
            return Err(product_transaction_err(error));
        }
        if generation == 0 {
            Ok(None)
        } else {
            omegat_team::pending_transaction_receipt(&props, generation)
                .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))
        }
    }

    fn set_entry_product_transaction(
        &mut self,
        params: Value,
        cancellation: &CancellationToken,
    ) -> std::result::Result<Value, (i32, String)> {
        let root = self.session()?.props.root.clone();
        let (generation, batch_id) = transaction_scope(&params, &root)?;
        let entry: SetEntryParams = serde_json::from_value(params).map_err(invalid)?;
        let session = self.session_mut()?;
        let checkpoint = session.checkpoint();
        let updated = session.set_entry(&entry).map_err(core_err)?;
        let props = session.props.clone();
        let result = omegat_team::commit_product_transaction_cancellable(
            &props,
            "entry.set",
            cancellation,
            "entry.set.snapshot",
            generation,
            batch_id.as_deref(),
            |_| session.save().map_err(core_product_err),
        );
        if let Err(error) = result {
            session.restore_checkpoint(checkpoint);
            return Err(product_transaction_err(error));
        }
        let receipt = if generation == 0 {
            None
        } else {
            omegat_team::pending_transaction_receipt(&props, generation)
                .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))?
        };
        let mut value = serde_json::to_value(updated).unwrap();
        value
            .as_object_mut()
            .expect("entry set result is an object")
            .insert("receipt".into(), serde_json::to_value(receipt).unwrap());
        Ok(value)
    }

    fn dispatch(
        &mut self,
        method: &str,
        params: Value,
        cancellation: &CancellationToken,
    ) -> std::result::Result<Value, (i32, String)> {
        if cancellation.is_cancelled() {
            return Err((error_code::REQUEST_CANCELLED, "request cancelled".into()));
        }
        match method {
            "sys.version" => Ok(serde_json::to_value(version()).unwrap()),
            "sys.capabilities" => Ok(serde_json::to_value(capabilities()).unwrap()),
            "sys.plugins" => Ok(serde_json::to_value(self.plugins.list(None)).unwrap()),
            "markers.list" => Ok(serde_json::to_value(self.plugins.registered_markers()).unwrap()),
            "markers.query" => {
                let id = params
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or((error_code::INVALID_PARAMS, "marker id".into()))?;
                let marks =
                    self.plugins
                        .marker_marks(id, &params)
                        .map_err(|error| match error {
                            omegat_plugin::PluginError::NotFound(_) => {
                                (error_code::INVALID_PARAMS, error.to_string())
                            }
                            _ => (error_code::INTERNAL_ERROR, error.to_string()),
                        })?;
                Ok(json!({ "marks": marks }))
            }
            "prefs.get" => Ok(serde_json::to_value(&self.prefs).unwrap()),
            "prefs.set" => {
                if let Ok(mut p) = serde_json::from_value::<Preferences>(params) {
                    if p.config_dir.as_os_str().is_empty() {
                        p.config_dir = self.prefs.config_dir.clone();
                    }
                    p.normalize();
                    if let Some(s) = self.session.as_mut() {
                        s.prefs = p.clone();
                    }
                    self.prefs = p;
                    let _ = self.prefs.save();
                }
                Ok(serde_json::to_value(&self.prefs).unwrap())
            }
            "project.create" => {
                let p: CreateProjectParams = serde_json::from_value(params).map_err(invalid)?;
                let s = ProjectSession::create_with_filters(
                    &p,
                    self.prefs.clone(),
                    self.plugins.filter_registry(),
                )
                .map_err(core_err)?;
                let dto = s.props.to_dto();
                self.session = Some(s);
                Ok(serde_json::to_value(dto).unwrap())
            }
            "project.open" => {
                let p: OpenProjectParams = serde_json::from_value(params).map_err(invalid)?;
                let recovery_props =
                    omegat_core::properties::ProjectProperties::load(std::path::Path::new(&p.root))
                        .map_err(core_err)?;
                omegat_team::recover_interrupted_sync(&recovery_props).map_err(|error| {
                    (
                        error_code::INTERNAL_ERROR,
                        format!("team transaction recovery: {error}"),
                    )
                })?;
                let s = ProjectSession::open_with_filters(
                    std::path::Path::new(&p.root),
                    self.prefs.clone(),
                    self.plugins.filter_registry(),
                )
                .map_err(core_err)?;
                let dto = s.props.to_dto();
                self.session = Some(s);
                Ok(serde_json::to_value(dto).unwrap())
            }
            "project.close" => {
                let receipt = if self.session.is_some() {
                    self.save_product_transaction("project.close", &params, cancellation)?
                } else {
                    None
                };
                self.session = None;
                Ok(json!({"ok": true, "receipt": receipt}))
            }
            "project.save" => {
                let receipt =
                    self.save_product_transaction("project.save", &params, cancellation)?;
                Ok(json!({"ok": true, "receipt": receipt}))
            }
            "project.compile" => {
                let file = params.get("file").and_then(|v| v.as_str());
                let n = self
                    .session_mut()?
                    .compile_cancellable(file, cancellation)
                    .map_err(core_err)?;
                Ok(json!({"files": n}))
            }
            "project.reload" => {
                self.session_mut()?
                    .reload_cancellable(cancellation)
                    .map_err(core_err)?;
                let list: Vec<EntryDto> = self
                    .session()?
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| e.to_dto(i))
                    .collect();
                Ok(
                    json!({"ok": true, "entries": list.len(), "props": self.session()?.props.to_dto()}),
                )
            }
            "project.external-refresh" => {
                self.session_mut()?
                    .refresh_external_cancellable(cancellation)
                    .map_err(core_err)?;
                Ok(external_refresh_result(self.session()?))
            }
            "project.props" => Ok(serde_json::to_value(self.session()?.props.to_dto()).unwrap()),
            "project.update" => {
                let s = self.session_mut()?;
                if let Some(sl) = params.get("source_lang").and_then(|v| v.as_str()) {
                    s.props.source_lang = sl.to_string();
                }
                if let Some(tl) = params.get("target_lang").and_then(|v| v.as_str()) {
                    s.props.target_lang = tl.to_string();
                }
                if let Some(seg) = params.get("sentence_segment").and_then(|v| v.as_bool()) {
                    s.props.sentence_seg = seg;
                }
                s.props.write().map_err(core_err)?;
                Ok(serde_json::to_value(s.props.to_dto()).unwrap())
            }
            "team.mapping" => {
                let s = self.session_mut()?;
                let repos = params
                    .get("repositories")
                    .cloned()
                    .unwrap_or(Value::Array(vec![]));
                let parsed: Vec<omegat_core::properties::RepositoryDef> =
                    serde_json::from_value(repos).map_err(invalid)?;
                s.props.repositories = parsed;
                s.props.write().map_err(core_err)?;
                Ok(json!({"ok": true, "repositories": s.props.to_dto().repositories}))
            }
            "entry.list" => {
                let s = self.session()?;
                let list: Vec<EntryDto> = s
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| e.to_dto(i))
                    .collect();
                Ok(serde_json::to_value(list).unwrap())
            }
            "entry.get" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let s = self.session()?;
                let e = s
                    .entries
                    .get(index)
                    .ok_or((error_code::INVALID_PARAMS, "index".into()))?;
                Ok(serde_json::to_value(e.to_dto(index)).unwrap())
            }
            "entry.set" => self.set_entry_product_transaction(params, cancellation),
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
                omegat_core::glossary::append_entry(
                    &s.props.glossary_file,
                    source,
                    target,
                    comment,
                )
                .map_err(|e| (error_code::IO, e.to_string()))?;
                s.glossary = omegat_core::glossary::load_glossary(&s.props.glossary_file);
                Ok(json!({"ok": true}))
            }
            "search.run" => {
                let p: SearchParams = serde_json::from_value(params).map_err(invalid)?;
                let hits = self
                    .session()?
                    .search_cancellable(&p, cancellation)
                    .ok_or((error_code::REQUEST_CANCELLED, "request cancelled".into()))?;
                Ok(serde_json::to_value(hits).unwrap())
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
            "spell.check" => {
                let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(serde_json::to_value(self.session()?.spell.misspelled_tokens(text)).unwrap())
            }
            "tmx.export" => {
                let level = params
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("omegat");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                let s = self.session()?;
                let xml = s
                    .tmx
                    .to_xml_level(&s.props.source_lang, &s.props.target_lang, level);
                if !dest.is_empty() {
                    std::fs::write(dest, &xml).map_err(|e| (error_code::IO, e.to_string()))?;
                }
                Ok(json!({"xml": xml, "level": level}))
            }
            "languagetool.check" => {
                let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let url = (!self.prefs.languagetool_url.is_empty())
                    .then(|| self.prefs.languagetool_url.clone());
                let lang = self
                    .session()
                    .map(|s| s.props.target_lang.clone())
                    .unwrap_or_else(|_| "en".into());
                let issues = omegat_core::languagetool::check_cancellable(
                    url.as_deref(),
                    text,
                    &lang,
                    0,
                    "",
                    cancellation,
                )
                .ok_or((error_code::REQUEST_CANCELLED, "request cancelled".into()))?;
                Ok(serde_json::to_value(issues).unwrap())
            }
            "finder.run" => {
                let xml = params
                    .get("xml")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        (!self.prefs.finder_xml.is_empty())
                            .then_some(self.prefs.finder_xml.as_str())
                    })
                    .unwrap_or("");
                let sel = params
                    .get("selection")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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
                let (transaction_generation, transaction_batch_id) =
                    transaction_scope(&params, &self.session()?.props.root)?;
                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let rebind_key = params
                    .get("rebind_key")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .map(serde_json::from_value::<EntryKeyDto>)
                    .transpose()
                    .map_err(invalid)?;
                if rebind_key.as_ref().is_some_and(|key| {
                    !self.session().is_ok_and(|session| {
                        session.entries.iter().any(|entry| entry.key() == *key)
                    })
                }) {
                    return Err((
                        error_code::INVALID_PARAMS,
                        "team conflict rebind key is no longer available".into(),
                    ));
                }
                let side = params
                    .get("side")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ours");
                let translation = params.get("translation").and_then(|v| v.as_str());
                let left = omegat_team::resolve_for_key_cancellable_scoped(
                    &self.session()?.props,
                    source,
                    rebind_key.as_ref(),
                    side,
                    translation,
                    cancellation,
                    transaction_generation,
                    transaction_batch_id.as_deref(),
                )
                .map_err(|error| match error {
                    omegat_team::TeamError::Cancelled => {
                        (error_code::REQUEST_CANCELLED, "request cancelled".into())
                    }
                    other => (error_code::TEAM_CONFLICT, other.to_string()),
                })?;
                let receipt = if transaction_generation == 0 {
                    None
                } else {
                    omegat_team::pending_transaction_receipt(
                        &self.session()?.props,
                        transaction_generation,
                    )
                    .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))?
                };
                Ok(json!({
                    "conflicts": left,
                    "rebind_key": rebind_key,
                    "receipt": receipt,
                }))
            }
            "wiki.import" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = &self.session()?.props.source_dir;
                let n = omegat_core::wiki::import_wiki(std::path::Path::new(src), dest)
                    .map_err(core_err)?;
                Ok(json!({"files": n}))
            }
            "med.open" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                omegat_core::wiki::open_med(std::path::Path::new(src), std::path::Path::new(dest))
                    .map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            "project.convert" => {
                let src = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                let sl = params
                    .get("source_lang")
                    .and_then(|v| v.as_str())
                    .unwrap_or("en");
                let tl = params
                    .get("target_lang")
                    .and_then(|v| v.as_str())
                    .unwrap_or("fr");
                omegat_core::wiki::convert_project(
                    std::path::Path::new(src),
                    std::path::Path::new(dest),
                    sl,
                    tl,
                )
                .map_err(core_err)?;
                Ok(json!({"ok": true}))
            }
            "aligner.configure" => {
                if params
                    .get("persist")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    if let Some(algo) = params.get("algo").and_then(|v| v.as_str()) {
                        self.prefs.aligner_algorithm = algo.to_string();
                    }
                    if let Some(calc) = params.get("calculator").and_then(|v| v.as_str()) {
                        self.prefs.aligner_calculator = calc.to_string();
                    }
                    if let Some(counter) = params.get("counter").and_then(|v| v.as_str()) {
                        self.prefs.aligner_counter = counter.to_string();
                    }
                    if let Some(seg) = params.get("segment").and_then(|v| v.as_bool()) {
                        self.prefs.aligner_segment = seg;
                    }
                    if let Some(rt) = params.get("remove_tags").and_then(|v| v.as_bool()) {
                        self.prefs.aligner_remove_tags = rt;
                    }
                    if let Some(sl) = params.get("source_lang").and_then(|v| v.as_str()) {
                        self.prefs.aligner_source_lang = sl.to_string();
                    }
                    if let Some(tl) = params.get("target_lang").and_then(|v| v.as_str()) {
                        self.prefs.aligner_target_lang = tl.to_string();
                    }
                    if let Some(d) = params.get("source_dir").and_then(|v| v.as_str()) {
                        self.prefs.aligner_last_source_dir = d.to_string();
                    }
                    if let Some(d) = params.get("target_dir").and_then(|v| v.as_str()) {
                        self.prefs.aligner_last_target_dir = d.to_string();
                    }
                    let _ = self.prefs.save();
                }
                Ok(json!({
                    "modes":["heapwise","parsewise","id"],
                    "algos":["viterbi","forward-backward"],
                    "counters":["char","word"],
                    "calculators":["normal","poisson"],
                    "algo": self.prefs.aligner_algorithm,
                    "calculator": self.prefs.aligner_calculator,
                    "counter": self.prefs.aligner_counter,
                    "segment": self.prefs.aligner_segment,
                    "remove_tags": self.prefs.aligner_remove_tags,
                    "source_lang": self.prefs.aligner_source_lang,
                    "target_lang": self.prefs.aligner_target_lang,
                    "source_dir": self.prefs.aligner_last_source_dir,
                    "target_dir": self.prefs.aligner_last_target_dir
                }))
            }
            "stats.get" => Ok(serde_json::to_value(self.session()?.stats()).unwrap()),
            "issues.list" => {
                let issues = self
                    .session()?
                    .issues_cancellable(cancellation)
                    .ok_or((error_code::REQUEST_CANCELLED, "request cancelled".into()))?;
                Ok(serde_json::to_value(issues).unwrap())
            }
            "filters.options" => {
                let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let reg = self.plugins.filter_registry();
                let Some(f) = reg.by_id(id) else {
                    return Err((error_code::INVALID_PARAMS, format!("unknown filter {id}")));
                };
                Ok(json!({
                    "id": f.id(),
                    "name": f.name(),
                    "masks": f.default_masks(),
                    "phase": f.phase(),
                    "options": {
                        "remove_tags": if self.prefs.remove_tags { "true" } else { "false" },
                        "preserve_spaces": self.prefs.filter_option(id, "preserve_spaces").unwrap_or("true"),
                        "file_context": self.prefs.filter_option(id, "file_context").unwrap_or(""),
                    }
                }))
            }
            "script.slots" => {
                let root = params
                    .get("root")
                    .and_then(|v| v.as_str())
                    .unwrap_or(self.prefs.script_dir.as_str());
                Ok(json!({ "slots": omegat_script::list_slots(std::path::Path::new(root)) }))
            }
            "script.slot" => {
                let slot = params.get("slot").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let root = std::path::Path::new(&self.prefs.script_dir);
                let source = self
                    .prefs
                    .script_slots
                    .get((slot as usize).saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                let src = if !source.is_empty() {
                    source
                } else {
                    let path = root.join(format!("slot{slot:02}.js"));
                    std::fs::read_to_string(path).unwrap_or_else(|_| "null".into())
                };
                self.dispatch(
                    "script.run",
                    json!({ "source": src, "index": index }),
                    cancellation,
                )
            }
            "project.import" => {
                let files = params
                    .get("files")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let dest = self.session()?.props.source_dir.clone();
                std::fs::create_dir_all(&dest).map_err(|e| (error_code::IO, e.to_string()))?;
                let mut copied = 0usize;
                for f in files {
                    let Some(src) = f.as_str() else { continue };
                    let name = std::path::Path::new(src).file_name().unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    std::fs::copy(src, dest.join(name))
                        .map_err(|e| (error_code::IO, e.to_string()))?;
                    copied += 1;
                }
                self.session_mut()?.reload().map_err(core_err)?;
                Ok(json!({ "copied": copied }))
            }
            "filters.list" => {
                let list: Vec<FilterInfoDto> = self
                    .plugins
                    .filter_registry()
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
            "filters.parse" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let id = params.get("id").and_then(|v| v.as_str());
                let reg = self.plugins.filter_registry();
                let filter = if let Some(id) = id {
                    reg.by_id(id)
                } else {
                    reg.for_path(std::path::Path::new(path))
                }
                .ok_or((error_code::FILTER, format!("no filter for {path}")))?;
                let parsed = filter
                    .parse_cancellable(
                        std::path::Path::new(path),
                        &omegat_filters::FilterContext::default(),
                        &|| cancellation.is_cancelled(),
                    )
                    .map_err(|error| match error {
                        omegat_filters::FilterError::Cancelled => {
                            (error_code::REQUEST_CANCELLED, "request cancelled".into())
                        }
                        other => core_err(other.into()),
                    })?;
                let segments: Vec<_> = parsed
                    .segments
                    .iter()
                    .map(|s| json!({"id": s.id, "source": s.source}))
                    .collect();
                Ok(json!({"id": filter.id(), "segments": segments}))
            }
            "mt.query" => {
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let engine = params
                    .get("engine")
                    .and_then(|v| v.as_str())
                    .unwrap_or("mymemory");
                let r = self
                    .session()?
                    .mt_cancellable(index, engine, cancellation)
                    .map_err(core_err)?;
                Ok(serde_json::to_value(r).unwrap())
            }
            "dict.query" => {
                let word = params.get("word").and_then(|v| v.as_str()).unwrap_or("");
                let hits = self
                    .session()?
                    .dict_cancellable(word, cancellation)
                    .ok_or((error_code::REQUEST_CANCELLED, "request cancelled".into()))?;
                Ok(serde_json::to_value(hits).unwrap())
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
                let (transaction_generation, transaction_batch_id) =
                    transaction_scope(&params, &self.session()?.props.root)?;
                let s = self.session()?;
                match omegat_team::sync_cancellable_scoped(
                    &s.props,
                    cancellation,
                    transaction_generation,
                    transaction_batch_id.as_deref(),
                ) {
                    Ok(r) => {
                        let receipt = if transaction_generation == 0 {
                            None
                        } else {
                            omegat_team::pending_transaction_receipt(
                                &s.props,
                                transaction_generation,
                            )
                            .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))?
                        };
                        Ok(json!({
                            "action": r.action,
                            "message": r.message,
                            "conflicts": r.conflicts,
                            "receipt": receipt,
                        }))
                    }
                    Err(omegat_team::TeamError::Cancelled) => {
                        Err((error_code::REQUEST_CANCELLED, "request cancelled".into()))
                    }
                    Err(omegat_team::TeamError::Conflict(msg)) => {
                        Err((error_code::TEAM_CONFLICT, msg))
                    }
                    Err(e) => Err((error_code::INTERNAL_ERROR, e.to_string())),
                }
            }
            "team.commit" => {
                let (transaction_generation, transaction_batch_id) =
                    transaction_scope(&params, &self.session()?.props.root)?;
                let which = params
                    .get("which")
                    .and_then(|v| v.as_str())
                    .unwrap_or("target");
                let r = omegat_team::commit_project_files_cancellable_scoped(
                    &self.session()?.props,
                    which,
                    cancellation,
                    transaction_generation,
                    transaction_batch_id.as_deref(),
                )
                .map_err(|error| match error {
                    omegat_team::TeamError::Cancelled => {
                        (error_code::REQUEST_CANCELLED, "request cancelled".into())
                    }
                    other => (error_code::INTERNAL_ERROR, other.to_string()),
                })?;
                let receipt = if transaction_generation == 0 {
                    None
                } else {
                    omegat_team::pending_transaction_receipt(
                        &self.session()?.props,
                        transaction_generation,
                    )
                    .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))?
                };
                Ok(json!({
                    "action": r.action,
                    "message": r.message,
                    "receipt": receipt,
                }))
            }
            "script.run" => {
                let src = params
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("null");
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
                                key: Some(e.key()),
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
                        let _ = omegat_core::glossary::append_entry(
                            &s.props.glossary_file,
                            src,
                            tgt,
                            cmt,
                        );
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
                let mode = params
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("parsewise");
                let algo = params
                    .get("algo")
                    .and_then(|v| v.as_str())
                    .unwrap_or("viterbi");
                let counter = params
                    .get("counter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("word");
                let calculator = params
                    .get("calculator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("normal");
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
                    segment: params
                        .get("segment")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
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
                let tmx = omegat_core::align::align_files_cfg_cancellable(
                    std::path::Path::new(source),
                    std::path::Path::new(target),
                    &sl,
                    &tl,
                    &cfg,
                    cancellation,
                )
                .map_err(core_err)?;
                if cancellation.is_cancelled() {
                    return Err((error_code::REQUEST_CANCELLED, "request cancelled".into()));
                }
                if !dest.is_empty() {
                    omegat_core::align::write_aligned_tmx_cancellable(
                        &tmx,
                        std::path::Path::new(dest),
                        &sl,
                        &tl,
                        cancellation,
                    )
                    .map_err(core_err)?;
                }
                let pairs: Vec<_> = tmx
                    .entries
                    .iter()
                    .map(|e| json!({"source": e.source, "target": e.translation}))
                    .collect();
                let beads: Vec<_> = tmx
                    .entries
                    .iter()
                    .map(|entry| {
                        mutable_bead_json(
                            &omegat_core::align::MutableBead::new(
                                entry.source.clone(),
                                entry.translation.clone(),
                            ),
                            &sl,
                            &tl,
                        )
                    })
                    .collect();
                Ok(json!({"ok": true, "pairs": pairs, "beads": beads, "count": pairs.len()}))
            }
            "align.edit" => {
                let action = params
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("merge");
                let index = params.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let side = params
                    .get("side")
                    .and_then(|v| v.as_str())
                    .map(omegat_core::align::AlignSide::from_name)
                    .unwrap_or(omegat_core::align::AlignSide::Both);
                let source_language = params
                    .get("source_lang")
                    .and_then(Value::as_str)
                    .unwrap_or("en");
                let target_language = params
                    .get("target_lang")
                    .and_then(Value::as_str)
                    .unwrap_or("fr");
                if let Some(raw_beads) = params.get("beads").and_then(Value::as_array) {
                    let beads: Vec<_> = raw_beads.iter().map(mutable_bead_from_json).collect();
                    let indexes: Vec<usize> = params
                        .get("indexes")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_u64)
                                .map(|value| value as usize)
                                .collect()
                        })
                        .unwrap_or_else(|| vec![index]);
                    let has_row_span = params.get("start_row").is_some();
                    let start_row =
                        params.get("start_row").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let end_row = params
                        .get("end_row")
                        .and_then(Value::as_u64)
                        .unwrap_or(start_row as u64) as usize;
                    let mut restored_selection = None;
                    let next = match action {
                        "merge" if has_row_span => omegat_core::align::merge_bead_row_span(
                            &beads,
                            start_row,
                            end_row,
                            side,
                            if matches!(side, omegat_core::align::AlignSide::Target) {
                                target_language
                            } else {
                                source_language
                            },
                        ),
                        "merge" => omegat_core::align::merge_beads(&beads, index, side),
                        "up" if has_row_span => omegat_core::align::move_bead_row_span(
                            &beads, start_row, end_row, side, -1,
                        ),
                        "up" => omegat_core::align::move_bead_side(
                            &beads,
                            index,
                            index.saturating_sub(1),
                            side,
                        ),
                        "down" if has_row_span => omegat_core::align::move_bead_row_span(
                            &beads, start_row, end_row, side, 1,
                        ),
                        "down" => omegat_core::align::move_bead_side(
                            &beads,
                            index,
                            (index + 1).min(beads.len().saturating_sub(1)),
                            side,
                        ),
                        "move-to-row" => {
                            let result = omegat_core::align::move_bead_row_span_to_with_selection(
                                &beads,
                                start_row,
                                end_row,
                                side,
                                params
                                    .get("target_row")
                                    .and_then(Value::as_i64)
                                    .unwrap_or(start_row as i64)
                                    as isize,
                            );
                            restored_selection = result.selection;
                            result.beads
                        }
                        "accepted" => {
                            restored_selection =
                                omegat_core::align::selection_after_bead_status(&beads, &indexes);
                            omegat_core::align::set_bead_status(
                                &beads,
                                &indexes,
                                omegat_core::align::BeadStatus::Accepted,
                            )
                        }
                        "needs-review" => {
                            restored_selection =
                                omegat_core::align::selection_after_bead_status(&beads, &indexes);
                            omegat_core::align::set_bead_status(
                                &beads,
                                &indexes,
                                omegat_core::align::BeadStatus::NeedsReview,
                            )
                        }
                        "clear-status" => {
                            restored_selection =
                                omegat_core::align::selection_after_bead_status(&beads, &indexes);
                            omegat_core::align::set_bead_status(
                                &beads,
                                &indexes,
                                omegat_core::align::BeadStatus::Default,
                            )
                        }
                        "keep-all" => omegat_core::align::set_beads_enabled(&beads, None, true),
                        "keep-none" => omegat_core::align::set_beads_enabled(&beads, None, false),
                        "keep" => omegat_core::align::set_beads_enabled(
                            &beads,
                            Some(&indexes),
                            params
                                .get("enabled")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        ),
                        "toggle-keep" => omegat_core::align::toggle_beads_enabled(&beads, &indexes),
                        "split" => {
                            let line_index = params
                                .get("line_index")
                                .and_then(Value::as_u64)
                                .unwrap_or(0) as usize;
                            let mut parts: Vec<String> = params
                                .get("lines")
                                .and_then(Value::as_array)
                                .map(|values| {
                                    values
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .map(str::to_string)
                                        .collect()
                                })
                                .unwrap_or_default();
                            if parts.len() < 2 {
                                let line = match side {
                                    omegat_core::align::AlignSide::Source => beads
                                        .get(index)
                                        .and_then(|bead| bead.source_lines.get(line_index)),
                                    omegat_core::align::AlignSide::Target => beads
                                        .get(index)
                                        .and_then(|bead| bead.target_lines.get(line_index)),
                                    omegat_core::align::AlignSide::Both => None,
                                }
                                .and_then(Option::as_deref)
                                .unwrap_or("");
                                if let Some((left, right)) = line.rsplit_once(' ') {
                                    parts = vec![left.to_string(), right.to_string()];
                                }
                            }
                            omegat_core::align::split_bead_line(
                                &beads, index, side, line_index, &parts,
                            )
                        }
                        "replace-span" => {
                            let replacement = params
                                .get("lines")
                                .and_then(Value::as_array)
                                .map(|values| {
                                    values
                                        .iter()
                                        .map(|value| value.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default();
                            omegat_core::align::replace_bead_row_span(
                                &beads,
                                start_row,
                                end_row,
                                side,
                                replacement,
                            )
                        }
                        "pinpoint" => {
                            let end_side = params
                                .get("end_side")
                                .and_then(Value::as_str)
                                .map(omegat_core::align::AlignSide::from_name)
                                .unwrap_or(omegat_core::align::AlignSide::Both);
                            if has_row_span {
                                omegat_core::align::pinpoint_align_rows(
                                    &beads,
                                    (start_row, side),
                                    (end_row, end_side),
                                )
                            } else {
                                let end_index = params
                                    .get("end_index")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(index as u64)
                                    as usize;
                                omegat_core::align::pinpoint_align(
                                    &beads,
                                    (index, side),
                                    (end_index, end_side),
                                )
                            }
                        }
                        "realign-pending" => omegat_core::align::realign_pending(
                            &beads,
                            match params.get("algo").and_then(Value::as_str) {
                                Some("forward-backward") => {
                                    omegat_core::align::AlignAlgo::ForwardBackward
                                }
                                _ => omegat_core::align::AlignAlgo::Viterbi,
                            },
                        )
                        .map_err(core_err)?,
                        _ => beads,
                    };
                    let pairs: Vec<_> = next
                        .iter()
                        .map(|bead| {
                            json!({
                                "source": bead.source_text(source_language),
                                "target": bead.target_text(target_language)
                            })
                        })
                        .collect();
                    let response_beads: Vec<_> = next
                        .iter()
                        .map(|bead| mutable_bead_json(bead, source_language, target_language))
                        .collect();
                    return Ok(json!({
                        "pairs": pairs,
                        "beads": response_beads,
                        "row_count": omegat_core::align::bead_rows(&next).len(),
                        "selection": restored_selection
                    }));
                }
                let raw = params
                    .get("pairs")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let pairs: Vec<(String, String)> = raw
                    .iter()
                    .map(|v| {
                        (
                            v.get("source")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string(),
                            v.get("target")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string(),
                        )
                    })
                    .collect();
                let next = omegat_core::align::edit_pairs_sided(&pairs, action, index, side);
                Ok(json!({
                    "pairs": next.iter().map(|(s,t)| json!({"source": s, "target": t})).collect::<Vec<_>>()
                }))
            }
            "align.write" => {
                let dest = params.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                if dest.is_empty() {
                    return Err((-32602, "align.write requires dest".into()));
                }
                let source_lang = params
                    .get("source_lang")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.session().ok().map(|s| s.props.source_lang.clone()))
                    .unwrap_or_else(|| "en".into());
                let target_lang = params
                    .get("target_lang")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.session().ok().map(|s| s.props.target_lang.clone()))
                    .unwrap_or_else(|| "fr".into());
                let pairs: Vec<(String, String)> =
                    if let Some(raw) = params.get("beads").and_then(Value::as_array) {
                        let beads: Vec<_> = raw.iter().map(mutable_bead_from_json).collect();
                        omegat_core::align::beads_to_pairs(&beads, &source_lang, &target_lang)
                    } else {
                        params
                            .get("pairs")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .filter(|value| {
                                value
                                    .get("enabled")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(true)
                            })
                            .map(|value| {
                                (
                                    value
                                        .get("source")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                    value
                                        .get("target")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                )
                            })
                            .collect()
                    };
                omegat_core::align::write_aligned_pairs(
                    &pairs,
                    std::path::Path::new(dest),
                    &source_lang,
                    &target_lang,
                )
                .map_err(core_err)?;
                Ok(json!({"ok": true, "count": pairs.len(), "dest": dest}))
            }
            other => Err((
                error_code::METHOD_NOT_FOUND,
                format!("unknown method {other}"),
            )),
        }
    }

    fn session(&self) -> std::result::Result<&ProjectSession, (i32, String)> {
        self.session
            .as_ref()
            .ok_or((error_code::PROJECT_NOT_OPEN, "no project".into()))
    }
    fn session_mut(&mut self) -> std::result::Result<&mut ProjectSession, (i32, String)> {
        self.session
            .as_mut()
            .ok_or((error_code::PROJECT_NOT_OPEN, "no project".into()))
    }
}

fn invalid(e: serde_json::Error) -> (i32, String) {
    (error_code::INVALID_PARAMS, e.to_string())
}

fn core_err(e: omegat_core::CoreError) -> (i32, String) {
    let code = match e {
        omegat_core::CoreError::Cancelled => error_code::REQUEST_CANCELLED,
        omegat_core::CoreError::OptimisticLock(_) => error_code::OPTIMISTIC_LOCK,
        omegat_core::CoreError::ProjectNotOpen => error_code::PROJECT_NOT_OPEN,
        omegat_core::CoreError::Io(_) => error_code::IO,
        omegat_core::CoreError::Filter(_) => error_code::FILTER,
        omegat_core::CoreError::TagValidation(_) => error_code::TAG_VALIDATION,
        _ => error_code::INTERNAL_ERROR,
    };
    (code, e.to_string())
}

fn core_product_err(error: omegat_core::CoreError) -> omegat_team::TeamError {
    match error {
        omegat_core::CoreError::Io(error) => omegat_team::TeamError::Io(error),
        omegat_core::CoreError::Cancelled => omegat_team::TeamError::Cancelled,
        other => omegat_team::TeamError::Command(other.to_string()),
    }
}

fn product_transaction_err(error: omegat_team::TeamError) -> (i32, String) {
    match error {
        omegat_team::TeamError::Cancelled => {
            (error_code::REQUEST_CANCELLED, "request cancelled".into())
        }
        omegat_team::TeamError::Io(error) => (error_code::IO, error.to_string()),
        other => (
            error_code::INTERNAL_ERROR,
            format!("product transaction: {other}"),
        ),
    }
}

fn external_refresh_result(session: &ProjectSession) -> Value {
    let entry_list: Vec<EntryDto> = session
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| entry.to_dto(index))
        .collect();
    json!({
        "ok": true,
        "entries": entry_list.len(),
        "entry_list": entry_list,
        "props": session.props.to_dto(),
        "stats": session.stats(),
    })
}

fn refresh_journal_err(error: String) -> (i32, String) {
    (
        error_code::INTERNAL_ERROR,
        format!("external refresh journal: {error}"),
    )
}

fn pending_transaction_envelopes(
    config_dir: &std::path::Path,
    props: &omegat_core::properties::ProjectProperties,
    app_instance: &str,
    generation: u64,
) -> std::result::Result<Vec<Value>, (i32, String)> {
    let mut envelopes = Vec::new();
    if let Some(receipt) = omegat_team::pending_transaction_receipt(props, generation)
        .map_err(|error| (error_code::INTERNAL_ERROR, error.to_string()))?
    {
        envelopes.push(serde_json::to_value(receipt).map_err(|error| {
            (
                error_code::INTERNAL_ERROR,
                format!("serialize product transaction receipt: {error}"),
            )
        })?);
    }
    envelopes.extend(
        refresh_journal::pending(config_dir, &props.root, app_instance, generation)
            .map_err(refresh_journal_err)?
            .into_iter()
            // The refresh journal owns its internal FIFO. Only its durable
            // head may compete with the product-receipt head; exposing a tail
            // here could let a newer row bypass an unacknowledged refresh
            // after the head transitions to sidecar_committed.
            .take(1)
            .map(|envelope| {
                serde_json::to_value(envelope).map_err(|error| {
                    (
                        error_code::INTERNAL_ERROR,
                        format!("serialize refresh transaction receipt: {error}"),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?,
    );
    envelopes.sort_by(|left, right| {
        left.get("updated_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            .cmp(
                &right
                    .get("updated_unix_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(u64::MAX),
            )
            .then_with(|| {
                left.get("batch_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(right.get("batch_id").and_then(Value::as_str).unwrap_or(""))
            })
            .then_with(|| {
                left.pointer("/payload/operation")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(
                        right
                            .pointer("/payload/operation")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    )
            })
    });
    Ok(envelopes)
}

fn transaction_scope(
    params: &Value,
    session_root: &std::path::Path,
) -> std::result::Result<(u64, Option<String>), (i32, String)> {
    if let Some(root) = params.get("transaction_project_root") {
        let root = root.as_str().filter(|value| !value.is_empty()).ok_or((
            error_code::INVALID_PARAMS,
            "transaction project root must be a non-empty string".into(),
        ))?;
        let normalized =
            |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if normalized(std::path::Path::new(root)) != normalized(session_root) {
            return Err((
                error_code::INVALID_PARAMS,
                "transaction project root is not the open project".into(),
            ));
        }
    }
    let generation = params
        .get("transaction_generation")
        .map(|value| {
            value.as_u64().ok_or((
                error_code::INVALID_PARAMS,
                "transaction generation must be an unsigned integer".into(),
            ))
        })
        .transpose()?
        .unwrap_or(0);
    let batch_id = params
        .get("transaction_batch_id")
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or((
                    error_code::INVALID_PARAMS,
                    "transaction batch id must be a non-empty string".into(),
                ))
        })
        .transpose()?;
    if generation != 0 && batch_id.is_none() {
        return Err((
            error_code::INVALID_PARAMS,
            "transaction generation requires a batch id".into(),
        ));
    }
    Ok((generation, batch_id))
}

fn transaction_receipt_scope(
    params: &Value,
    open_root: Option<&std::path::Path>,
    require_batch_id: bool,
) -> std::result::Result<
    (
        std::path::PathBuf,
        String,
        u64,
        Option<String>,
        Option<String>,
    ),
    (i32, String),
> {
    let root = params
        .get("root")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .ok_or((
            error_code::INVALID_PARAMS,
            "transaction receipt requires root".into(),
        ))?;
    let normalized =
        |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(open_root) = open_root {
        if normalized(&root) != normalized(open_root) {
            return Err((
                error_code::INVALID_PARAMS,
                "transaction receipt root is not the open project".into(),
            ));
        }
    } else if !require_batch_id {
        return Err((error_code::PROJECT_NOT_OPEN, "no project".into()));
    }
    let app_instance = params
        .get("app_instance")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or((
            error_code::INVALID_PARAMS,
            "transaction receipt requires app_instance".into(),
        ))?;
    let generation = params
        .get("generation")
        .and_then(Value::as_u64)
        .filter(|generation| *generation != 0)
        .ok_or((
            error_code::INVALID_PARAMS,
            "transaction receipt requires a non-zero generation".into(),
        ))?;
    let batch_id = params
        .get("batch_id")
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or((
                    error_code::INVALID_PARAMS,
                    "transaction acknowledgement batch id must be non-empty".into(),
                ))
        })
        .transpose()?;
    if require_batch_id && batch_id.is_none() {
        return Err((
            error_code::INVALID_PARAMS,
            "transaction acknowledgement requires batch_id".into(),
        ));
    }
    let operation = params
        .get("operation")
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or((
                    error_code::INVALID_PARAMS,
                    "transaction acknowledgement operation must be non-empty".into(),
                ))
        })
        .transpose()?;
    if require_batch_id && operation.is_none() {
        return Err((
            error_code::INVALID_PARAMS,
            "transaction acknowledgement requires operation".into(),
        ));
    }
    Ok((root, app_instance, generation, batch_id, operation))
}

fn refresh_scope(
    params: &Value,
    open_root: Option<&std::path::Path>,
) -> std::result::Result<(std::path::PathBuf, String, u64), (i32, String)> {
    let session_root = open_root.ok_or((error_code::PROJECT_NOT_OPEN, "no project".into()))?;
    let root = params
        .get("root")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .ok_or((
            error_code::INVALID_PARAMS,
            "refresh journal requires root".into(),
        ))?;
    let normalized =
        |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if normalized(&root) != normalized(session_root) {
        return Err((
            error_code::INVALID_PARAMS,
            "refresh journal root is not the open project".into(),
        ));
    }
    let app_instance = params
        .get("app_instance")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or((
            error_code::INVALID_PARAMS,
            "refresh journal requires app_instance".into(),
        ))?
        .to_string();
    let generation = params.get("generation").and_then(Value::as_u64).ok_or((
        error_code::INVALID_PARAMS,
        "refresh journal requires generation".into(),
    ))?;
    Ok((root, app_instance, generation))
}

fn dispatch_refresh_journal(
    method: &str,
    params: Value,
    config_dir: &std::path::Path,
    open_root: Option<&std::path::Path>,
) -> Option<std::result::Result<Value, (i32, String)>> {
    if !method.starts_with("project.refresh.") && !method.starts_with("transaction.receipt.") {
        return None;
    }
    Some((|| {
        if method.starts_with("transaction.receipt.") {
            let require_batch_id = method == "transaction.receipt.ack";
            let (root, app_instance, generation, batch_id, operation) =
                transaction_receipt_scope(&params, open_root, require_batch_id)?;
            let props =
                omegat_core::properties::ProjectProperties::load(&root).map_err(core_err)?;
            return match method {
                "transaction.receipt.pending" => {
                    let mut envelopes = pending_transaction_envelopes(
                        config_dir,
                        &props,
                        &app_instance,
                        generation,
                    )?;
                    // Expose exactly one durable head. The Electron dispatcher
                    // asks again after its renderer acknowledgement, so neither
                    // backend can race or publish around an older receipt.
                    envelopes.truncate(1);
                    Ok(json!({ "envelopes": envelopes }))
                }
                "transaction.receipt.ack" => {
                    let batch_id = batch_id.as_deref().expect("required transaction batch id");
                    let operation = operation
                        .as_deref()
                        .expect("required transaction operation");
                    let pending = pending_transaction_envelopes(
                        config_dir,
                        &props,
                        &app_instance,
                        generation,
                    )?;
                    if let Some(index) = pending.iter().position(|envelope| {
                        envelope.get("batch_id").and_then(Value::as_str) == Some(batch_id)
                    }) {
                        if index != 0 {
                            let head = pending[0]
                                .get("batch_id")
                                .and_then(Value::as_str)
                                .unwrap_or("<invalid>");
                            return Err((
                                error_code::TEAM_CONFLICT,
                                format!("transaction receipt FIFO head is {head}, not {batch_id}"),
                            ));
                        }
                    }
                    let ack = if operation == "project.external-refresh" {
                        let outcome = params
                            .get("outcome")
                            .and_then(Value::as_str)
                            .filter(|value| {
                                matches!(*value, "succeeded" | "cancelled" | "coalesced")
                            })
                            .ok_or((
                                error_code::INVALID_PARAMS,
                                "refresh acknowledgement requires a terminal outcome".into(),
                            ))?;
                        refresh_journal::acknowledge(
                            config_dir,
                            &root,
                            &app_instance,
                            generation,
                            batch_id,
                            outcome,
                        )
                        .map_err(|error| (error_code::TEAM_CONFLICT, error))?
                    } else {
                        omegat_team::acknowledge_transaction_receipt(
                            &props, generation, batch_id, operation,
                        )
                        .map_err(|error| match error {
                            omegat_team::TeamError::Conflict(message) => {
                                (error_code::TEAM_CONFLICT, message)
                            }
                            other => (error_code::INTERNAL_ERROR, other.to_string()),
                        })?
                    };
                    Ok(json!({ "ack": ack }))
                }
                _ => Err((
                    error_code::METHOD_NOT_FOUND,
                    format!("unknown method {method}"),
                )),
            };
        }
        let (root, app_instance, generation) = refresh_scope(&params, open_root)?;
        match method {
            "project.refresh.enqueue" => {
                let paths = params
                    .get("paths")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                    .filter(|paths| !paths.is_empty())
                    .ok_or((
                        error_code::INVALID_PARAMS,
                        "refresh enqueue requires paths".into(),
                    ))?;
                let fingerprints = params
                    .get("fingerprints")
                    .cloned()
                    .and_then(|value| {
                        serde_json::from_value::<BTreeMap<String, Option<String>>>(value).ok()
                    })
                    .ok_or((
                        error_code::INVALID_PARAMS,
                        "refresh enqueue requires fingerprints".into(),
                    ))?;
                let sources = params
                    .get("sources")
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                    .filter(|sources| {
                        !sources.is_empty()
                            && sources
                                .iter()
                                .all(|source| matches!(source.as_str(), "native" | "sidecar"))
                    })
                    .ok_or((
                        error_code::INVALID_PARAMS,
                        "refresh enqueue requires native/sidecar sources".into(),
                    ))?;
                let batch = refresh_journal::enqueue(
                    config_dir,
                    &root,
                    &app_instance,
                    generation,
                    paths,
                    fingerprints,
                    sources,
                )
                .map_err(refresh_journal_err)?;
                Ok(json!({ "batch": batch }))
            }
            "project.refresh.discard" => {
                refresh_journal::discard(config_dir, &root, &app_instance)
                    .map_err(refresh_journal_err)?;
                Ok(json!({ "discarded": true }))
            }
            _ => Err((
                error_code::METHOD_NOT_FOUND,
                format!("unknown method {method}"),
            )),
        }
    })())
}

fn settle_external_refresh_journal(
    method: &str,
    params: &Value,
    config_dir: &std::path::Path,
    open_root: Option<&std::path::Path>,
    response_error_code: Option<i32>,
    response_result: Option<&Value>,
) -> Result<(), String> {
    if method != "project.external-refresh" {
        return Ok(());
    }
    let Some(batch_id) = params
        .get("transaction_batch_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let root = params
        .get("transaction_project_root")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "checkpoint requires transaction_project_root".to_string())?;
    let open_root = open_root.ok_or_else(|| "checkpoint has no open project".to_string())?;
    let normalized =
        |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if normalized(&root) != normalized(open_root) {
        return Err("checkpoint project root is not the open project".into());
    }
    let generation = params
        .get("transaction_generation")
        .and_then(Value::as_u64)
        .ok_or_else(|| "checkpoint requires transaction_generation".to_string())?;
    let app_instance = params
        .get("app_instance")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "checkpoint requires app_instance".to_string())?;
    match response_error_code {
        None => {
            let committed_result = response_result
                .ok_or_else(|| "successful refresh has no product result".to_string())?;
            if std::env::var("OMEGAT_TEST_ABORT_EXTERNAL_REFRESH_AT").as_deref()
                == Ok("before_atomic_publish")
            {
                std::process::abort();
            }
            refresh_journal::checkpoint_sidecar_commit(
                config_dir,
                &root,
                app_instance,
                generation,
                batch_id,
                committed_result,
            )?;
            if std::env::var("OMEGAT_TEST_ABORT_EXTERNAL_REFRESH_AT").as_deref()
                == Ok("after_atomic_publish")
            {
                std::process::abort();
            }
            Ok(())
        }
        Some(error_code::REQUEST_CANCELLED) => refresh_journal::request_cancelled(
            config_dir,
            &root,
            app_instance,
            generation,
            batch_id,
        )
        .map(|_| ()),
        Some(_) => Ok(()),
    }
}

fn request_key(id: &Value) -> String {
    serde_json::to_string(id).unwrap_or_else(|_| "null".into())
}

fn writes_watched_project_input(method: &str) -> bool {
    matches!(
        method,
        "entry.set"
            | "project.save"
            | "project.compile"
            | "project.close"
            | "project.update"
            | "project.import"
            | "team.mapping"
            | "team.sync"
            | "team.commit"
            | "team.resolve"
            | "glossary.add"
            | "spell.ignore"
            | "spell.learn"
            | "wiki.import"
    )
}

fn plugin_marker_worker_main(
    library_path: &std::path::Path,
    marker_id: &str,
) -> std::result::Result<(), String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("cannot read marker worker input: {error}"))?;
    let input: Value = serde_json::from_str(&input)
        .map_err(|error| format!("invalid marker worker input: {error}"))?;
    let marks = omegat_plugin::run_marker_worker(library_path, marker_id, &input)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(io::stdout(), &json!({ "marks": marks }))
        .map_err(|error| format!("cannot write marker worker output: {error}"))
}

fn main() {
    let mut args = std::env::args_os();
    let _program = args.next();
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--plugin-marker-worker")) {
        let Some(library_path) = args.next() else {
            eprintln!("missing plugin library path");
            std::process::exit(2);
        };
        let Some(marker_id) = args.next() else {
            eprintln!("missing plugin marker id");
            std::process::exit(2);
        };
        if let Err(error) = plugin_marker_worker_main(
            std::path::Path::new(&library_path),
            &marker_id.to_string_lossy(),
        ) {
            eprintln!("{error}");
            std::process::exit(3);
        }
        return;
    }
    let _ = env_logger::try_init();
    let app_state = App::new();
    let refresh_config_dir = app_state.prefs.config_dir.clone();
    let app = Arc::new(Mutex::new(app_state));
    let open_project = Arc::new(Mutex::new(None::<std::path::PathBuf>));
    let refresh_journal_lock = Arc::new(Mutex::new(()));
    let cancellations = Arc::new(Mutex::new(HashMap::<String, CancellationToken>::new()));
    let (responses, response_lines) = std::sync::mpsc::channel::<String>();
    let (watch_commands, watch_worker) = project_watcher::spawn(responses.clone());
    let writer = thread::spawn(move || {
        let mut stdout = io::stdout();
        while let Ok(line) = response_lines.recv() {
            let _ = writeln!(stdout, "{line}");
            let _ = stdout.flush();
        }
    });
    let stdin = io::stdin();
    let mut workers = Vec::new();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse::err(Value::Null, error_code::PARSE_ERROR, e.to_string());
                let _ = responses.send(serde_json::to_string(&resp).unwrap());
                continue;
            }
        };
        if req.id.is_none() && req.method == "$/cancelRequest" {
            if let Some(id) = req.params.get("id") {
                if let Some(cancellation) =
                    cancellations.lock().unwrap().get(&request_key(id)).cloned()
                {
                    cancellation.cancel();
                }
            }
            continue;
        }
        if req.id.is_none() {
            continue;
        }
        let id = req.id.clone().unwrap_or(Value::Null);
        let key = request_key(&id);
        let cancellation = if let Some(progress_token) = req.params.get("progress_token").cloned() {
            let progress_responses = responses.clone();
            CancellationToken::with_checkpoint_observer(move |stage| {
                let notification = RpcNotification::new(
                    "$/progress",
                    json!({ "token": progress_token, "stage": stage }),
                );
                if let Ok(line) = serde_json::to_string(&notification) {
                    let _ = progress_responses.send(line);
                }
            })
        } else {
            CancellationToken::default()
        };
        cancellations
            .lock()
            .unwrap()
            .insert(key.clone(), cancellation.clone());
        let app = Arc::clone(&app);
        let open_project = Arc::clone(&open_project);
        let refresh_journal_lock = Arc::clone(&refresh_journal_lock);
        let refresh_config_dir = refresh_config_dir.clone();
        let cancellations = Arc::clone(&cancellations);
        let responses = responses.clone();
        let watch_commands = watch_commands.clone();
        workers.push(thread::spawn(move || {
            let project_lifecycle_method = req.method.clone();
            let transaction_params = req.params.clone();
            let project_input_write = writes_watched_project_input(&project_lifecycle_method);
            if project_input_write {
                let (ready, ready_rx) = std::sync::mpsc::sync_channel(0);
                let _ = watch_commands.send(project_watcher::WatchCommand::BeginWrite(ready));
                let _ = ready_rx.recv_timeout(std::time::Duration::from_secs(2));
            }
            let refresh_result = {
                let _journal = refresh_journal_lock.lock().unwrap();
                let active = open_project.lock().unwrap();
                dispatch_refresh_journal(
                    &req.method,
                    req.params.clone(),
                    &refresh_config_dir,
                    active.as_deref(),
                )
            };
            let scoped_external_refresh = project_lifecycle_method == "project.external-refresh"
                && transaction_params
                    .get("transaction_batch_id")
                    .and_then(Value::as_str)
                    .is_some();
            let (mut resp, checkpoint_result, checkpoint_settled) = match refresh_result {
                Some(Ok(result)) => (RpcResponse::ok(id.clone(), result), Ok(()), false),
                Some(Err((code, message))) => {
                    (RpcResponse::err(id.clone(), code, message), Ok(()), false)
                }
                None if scoped_external_refresh => {
                    // Hold the session and journal locks across candidate
                    // publication. Readers can observe neither the refreshed
                    // entry list nor its receipt until the atomic rename wins.
                    let _journal = refresh_journal_lock.lock().unwrap();
                    let active = open_project.lock().unwrap();
                    let mut app = app.lock().unwrap();
                    let response = app.handle_external_refresh_transactional(
                        id.clone(),
                        &cancellation,
                        |committed_result| {
                            settle_external_refresh_journal(
                                &project_lifecycle_method,
                                &transaction_params,
                                &refresh_config_dir,
                                active.as_deref(),
                                None,
                                Some(committed_result),
                            )
                        },
                    );
                    let cancellation_checkpoint = if response.error.as_ref().map(|error| error.code)
                        == Some(error_code::REQUEST_CANCELLED)
                    {
                        settle_external_refresh_journal(
                            &project_lifecycle_method,
                            &transaction_params,
                            &refresh_config_dir,
                            active.as_deref(),
                            Some(error_code::REQUEST_CANCELLED),
                            None,
                        )
                    } else {
                        Ok(())
                    };
                    (response, cancellation_checkpoint, true)
                }
                None => (
                    app.lock().unwrap().handle(req, &cancellation),
                    Ok(()),
                    false,
                ),
            };
            let checkpoint_result = if checkpoint_settled {
                checkpoint_result
            } else {
                let _journal = refresh_journal_lock.lock().unwrap();
                let active = open_project.lock().unwrap();
                settle_external_refresh_journal(
                    &project_lifecycle_method,
                    &transaction_params,
                    &refresh_config_dir,
                    active.as_deref(),
                    resp.error.as_ref().map(|error| error.code),
                    resp.result.as_ref(),
                )
            };
            if let Err(error) = checkpoint_result {
                resp = RpcResponse::err(
                    id.clone(),
                    error_code::INTERNAL_ERROR,
                    format!("external refresh checkpoint: {error}"),
                );
            }
            if project_input_write {
                let (ready, ready_rx) = std::sync::mpsc::sync_channel(0);
                let _ = watch_commands.send(project_watcher::WatchCommand::EndWrite(ready));
                let _ = ready_rx.recv_timeout(std::time::Duration::from_secs(2));
            }
            cancellations.lock().unwrap().remove(&key);
            if resp.error.is_none() {
                match project_lifecycle_method.as_str() {
                    "project.create" | "project.open" => {
                        if let Some(root) = resp
                            .result
                            .as_ref()
                            .and_then(|result| result.get("root"))
                            .and_then(Value::as_str)
                        {
                            *open_project.lock().unwrap() = Some(std::path::PathBuf::from(root));
                            let (ready, ready_rx) = std::sync::mpsc::sync_channel(0);
                            let _ = watch_commands.send(project_watcher::WatchCommand::Watch(
                                std::path::PathBuf::from(root),
                                ready,
                            ));
                            let _ = ready_rx.recv_timeout(std::time::Duration::from_secs(2));
                        }
                    }
                    "project.close" => {
                        *open_project.lock().unwrap() = None;
                        let _ = watch_commands.send(project_watcher::WatchCommand::Close);
                    }
                    _ => {}
                }
            }
            let _ = responses.send(serde_json::to_string(&resp).unwrap());
        }));
    }
    for worker in workers {
        let _ = worker.join();
    }
    let _ = watch_commands.send(project_watcher::WatchCommand::Shutdown);
    let _ = watch_worker.join();
    drop(responses);
    let _ = writer.join();
}
