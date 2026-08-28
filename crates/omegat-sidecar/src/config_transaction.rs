// SPDX-License-Identifier: GPL-3.0-or-later

//! Process-shared transactions for configuration products.
//!
//! Project products use the per-project `omegat-team` journal. Preferences,
//! filter/segmentation/aligner settings, and installed spelling dictionaries
//! instead share one config directory across every Electron process. This
//! module gives those writes their own durable FIFO and OS lock so a stale
//! sidecar cannot silently replace fields committed by another sidecar.

use fs2::FileExt;
use omegat_core::prefs::Preferences;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CONFIG_TRANSACTION_VERSION: u8 = 1;
const TRANSACTION_DIRECTORY: &str = "shared-config";
static BATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConfigTransactionStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigTransactionEnvelope {
    version: u8,
    config_dir: PathBuf,
    batch_id: String,
    operation: String,
    app_instance: String,
    owner_process_id: u32,
    status: ConfigTransactionStatus,
    payload: Value,
    result: Option<Value>,
    error: Option<String>,
    updated_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigTransactionJournal {
    version: u8,
    config_dir: PathBuf,
    batches: Vec<ConfigTransactionEnvelope>,
    updated_unix_ms: u128,
}

impl ConfigTransactionJournal {
    fn empty(config_dir: &Path) -> Self {
        Self {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            batches: Vec::new(),
            updated_unix_ms: unix_ms(),
        }
    }
}

struct ConfigTransactionLock {
    _file: File,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn transaction_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("transactions").join(TRANSACTION_DIRECTORY)
}

fn active_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("active.json")
}

fn history_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("history.ndjson")
}

fn sync_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("config transaction path has no parent: {}", path.display()))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "sync config transaction directory {}: {error}",
                parent.display()
            )
        })
}

fn acquire_lock(config_dir: &Path) -> Result<ConfigTransactionLock, String> {
    let directory = transaction_dir(config_dir);
    std::fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "create config transaction directory {}: {error}",
            directory.display()
        )
    })?;
    sync_parent(&directory)?;
    let path = directory.join("operation.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("open config transaction lock {}: {error}", path.display()))?;
    file.lock_exclusive()
        .map_err(|error| format!("lock shared config {}: {error}", path.display()))?;
    Ok(ConfigTransactionLock { _file: file })
}

fn read_journal(config_dir: &Path) -> Result<ConfigTransactionJournal, String> {
    let path = active_path(config_dir);
    if !path.is_file() {
        return Ok(ConfigTransactionJournal::empty(config_dir));
    }
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "read config transaction journal {}: {error}",
            path.display()
        )
    })?;
    let journal: ConfigTransactionJournal = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parse config transaction journal {}: {error}",
            path.display()
        )
    })?;
    if journal.version != CONFIG_TRANSACTION_VERSION
        || normalized(&journal.config_dir) != normalized(config_dir)
        || journal.batches.iter().any(|batch| {
            batch.version != CONFIG_TRANSACTION_VERSION
                || normalized(&batch.config_dir) != normalized(config_dir)
                || batch.batch_id.is_empty()
                || batch.operation.is_empty()
                || batch.status != ConfigTransactionStatus::Pending
        })
    {
        return Err(format!(
            "invalid config transaction journal {}",
            path.display()
        ));
    }
    Ok(journal)
}

fn persist_journal(config_dir: &Path, journal: &ConfigTransactionJournal) -> Result<(), String> {
    let path = active_path(config_dir);
    if journal.batches.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => sync_parent(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "remove config transaction journal {}: {error}",
                path.display()
            )),
        }
    } else {
        let bytes = serde_json::to_vec_pretty(journal)
            .map_err(|error| format!("serialize config transaction journal: {error}"))?;
        omegat_core::durable_file::replace(&path, &bytes).map_err(|error| {
            format!(
                "publish config transaction journal {}: {error}",
                path.display()
            )
        })
    }
}

fn read_history(config_dir: &Path) -> Result<Vec<ConfigTransactionEnvelope>, String> {
    let path = history_path(config_dir);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read config transaction history {}: {error}",
                path.display()
            ))
        }
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            let envelope: ConfigTransactionEnvelope =
                serde_json::from_str(line).map_err(|error| {
                    format!(
                        "parse config transaction history {} line {}: {error}",
                        path.display(),
                        index + 1
                    )
                })?;
            if envelope.version != CONFIG_TRANSACTION_VERSION
                || normalized(&envelope.config_dir) != normalized(config_dir)
                || envelope.status == ConfigTransactionStatus::Pending
            {
                return Err(format!(
                    "invalid config transaction history {} line {}",
                    path.display(),
                    index + 1
                ));
            }
            Ok(envelope)
        })
        .collect()
}

fn persist_history(config_dir: &Path, history: &[ConfigTransactionEnvelope]) -> Result<(), String> {
    let mut bytes = Vec::new();
    for envelope in history {
        serde_json::to_writer(&mut bytes, envelope)
            .map_err(|error| format!("serialize config transaction history: {error}"))?;
        bytes.push(b'\n');
    }
    let path = history_path(config_dir);
    omegat_core::durable_file::replace(&path, &bytes).map_err(|error| {
        format!(
            "publish config transaction history {}: {error}",
            path.display()
        )
    })
}

fn checkpoint(
    operation: &str,
    point: &str,
    envelope: &ConfigTransactionEnvelope,
) -> Result<(), String> {
    if std::env::var("OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION").as_deref() != Ok(operation)
        || std::env::var("OMEGAT_TEST_CONFIG_TRANSACTION_POINT").as_deref() != Ok(point)
    {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_CONFIG_TRANSACTION_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create config checkpoint directory: {error}"))?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
        .map_err(|error| format!("create config checkpoint {}: {error}", marker.display()))?;
    serde_json::to_writer(
        &mut file,
        &json!({
            "batch_id": envelope.batch_id,
            "operation": envelope.operation,
            "app_instance": envelope.app_instance,
            "owner_process_id": envelope.owner_process_id,
            "sidecar_process_id": std::process::id(),
            "point": point,
        }),
    )
    .map_err(|error| format!("write config checkpoint: {error}"))?;
    file.write_all(b"\n")
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("sync config checkpoint: {error}"))?;
    sync_parent(&marker)?;
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let target = target.as_object_mut().expect("object initialized above");
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// Return the recursive merge patch that turns `base` into `desired`.
///
/// A full preferences editor can therefore submit its draft without replacing
/// unrelated fields written since that editor loaded its original snapshot.
pub fn preference_patch(base: &Value, desired: &Value) -> Value {
    match (base, desired) {
        (Value::Object(base), Value::Object(desired)) => {
            let mut patch = Map::new();
            for (key, desired_value) in desired {
                match base.get(key) {
                    Some(base_value) if base_value == desired_value => {}
                    Some(base_value) => {
                        patch.insert(key.clone(), preference_patch(base_value, desired_value));
                    }
                    None => {
                        patch.insert(key.clone(), desired_value.clone());
                    }
                }
            }
            for key in base.keys() {
                if !desired.contains_key(key) {
                    patch.insert(key.clone(), Value::Null);
                }
            }
            Value::Object(patch)
        }
        _ => desired.clone(),
    }
}

fn apply_preferences_patch(config_dir: &Path, patch: &Value) -> Result<Value, String> {
    let current = Preferences::load_or_default(config_dir);
    let mut value = serde_json::to_value(current)
        .map_err(|error| format!("serialize current preferences: {error}"))?;
    merge_patch(&mut value, patch);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "config_dir".into(),
            Value::String(config_dir.to_string_lossy().into_owned()),
        );
    }
    let mut preferences: Preferences = serde_json::from_value(value)
        .map_err(|error| format!("merge shared preferences: {error}"))?;
    preferences.config_dir = config_dir.to_path_buf();
    preferences.normalize();
    preferences
        .save()
        .map_err(|error| format!("save shared preferences: {error}"))?;
    serde_json::to_value(preferences)
        .map_err(|error| format!("serialize merged preferences: {error}"))
}

fn aligner_patch(payload: &Value) -> Value {
    let mut patch = Map::new();
    for (source, destination) in [
        ("algo", "aligner_algorithm"),
        ("calculator", "aligner_calculator"),
        ("counter", "aligner_counter"),
        ("segment", "aligner_segment"),
        ("remove_tags", "aligner_remove_tags"),
        ("source_lang", "aligner_source_lang"),
        ("target_lang", "aligner_target_lang"),
        ("source_dir", "aligner_last_source_dir"),
        ("target_dir", "aligner_last_target_dir"),
    ] {
        if let Some(value) = payload.get(source) {
            patch.insert(destination.into(), value.clone());
        }
    }
    Value::Object(patch)
}

fn apply_operation(config_dir: &Path, operation: &str, payload: &Value) -> Result<Value, String> {
    match operation {
        "prefs.patch" => apply_preferences_patch(config_dir, payload),
        "aligner.configure" => {
            let preferences = apply_preferences_patch(config_dir, &aligner_patch(payload))?;
            Ok(json!({
                "modes": ["heapwise", "parsewise", "id"],
                "algos": ["viterbi", "forward-backward"],
                "counters": ["char", "word"],
                "calculators": ["normal", "poisson"],
                "algo": preferences["aligner_algorithm"],
                "calculator": preferences["aligner_calculator"],
                "counter": preferences["aligner_counter"],
                "segment": preferences["aligner_segment"],
                "remove_tags": preferences["aligner_remove_tags"],
                "source_lang": preferences["aligner_source_lang"],
                "target_lang": preferences["aligner_target_lang"],
                "source_dir": preferences["aligner_last_source_dir"],
                "target_dir": preferences["aligner_last_target_dir"],
                "preferences": preferences,
            }))
        }
        "spell.install" => {
            let lang = payload.get("lang").and_then(Value::as_str).unwrap_or("en");
            let destination = config_dir.join("spell").join("hunspell");
            let ok = omegat_core::spell::install_lang(lang, &destination)
                .map_err(|error| format!("install spelling dictionary {lang}: {error}"))?;
            Ok(json!({
                "ok": ok,
                "lang": lang,
                "dest": destination.display().to_string(),
            }))
        }
        _ => Err(format!(
            "unsupported config transaction operation {operation}"
        )),
    }
}

fn validate_identity(
    existing: &ConfigTransactionEnvelope,
    operation: &str,
    payload: &Value,
) -> Result<(), String> {
    if existing.operation == operation && existing.payload == *payload {
        Ok(())
    } else {
        Err(format!(
            "config transaction batch {} was reused for a different operation",
            existing.batch_id
        ))
    }
}

fn result_from_terminal(envelope: &ConfigTransactionEnvelope) -> Result<Value, String> {
    match envelope.status {
        ConfigTransactionStatus::Completed => envelope
            .result
            .clone()
            .ok_or_else(|| format!("config transaction {} has no result", envelope.batch_id)),
        ConfigTransactionStatus::Failed => Err(envelope
            .error
            .clone()
            .unwrap_or_else(|| format!("config transaction {} failed", envelope.batch_id))),
        ConfigTransactionStatus::Pending => Err(format!(
            "config transaction {} is still pending",
            envelope.batch_id
        )),
    }
}

fn drain_locked(
    config_dir: &Path,
    journal: &mut ConfigTransactionJournal,
    history: &mut Vec<ConfigTransactionEnvelope>,
) -> Result<(), String> {
    while let Some(pending) = journal.batches.first().cloned() {
        if let Some(terminal) = history.iter().find(|row| row.batch_id == pending.batch_id) {
            validate_identity(terminal, &pending.operation, &pending.payload)?;
        } else {
            let mut terminal = pending.clone();
            match apply_operation(config_dir, &pending.operation, &pending.payload) {
                Ok(result) => {
                    terminal.status = ConfigTransactionStatus::Completed;
                    terminal.result = Some(result);
                    terminal.error = None;
                }
                Err(error) => {
                    terminal.status = ConfigTransactionStatus::Failed;
                    terminal.result = None;
                    terminal.error = Some(error);
                }
            }
            terminal.updated_unix_ms = unix_ms();
            history.push(terminal.clone());
            persist_history(config_dir, history)?;
            checkpoint(&pending.operation, "after_history_append", &terminal)?;
        }
        journal.batches.remove(0);
        journal.updated_unix_ms = unix_ms();
        persist_journal(config_dir, journal)?;
    }
    Ok(())
}

fn fallback_identity() -> (String, String, u32) {
    let process_id = std::process::id();
    let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    (
        format!("sidecar-{process_id}"),
        format!("config-{process_id}-{}-{sequence}", unix_ms()),
        process_id,
    )
}

/// Parse and remove the config transaction identity attached by Electron.
pub fn take_scope(params: &mut Value) -> (String, String, u32) {
    let fallback = fallback_identity();
    let Some(object) = params.as_object_mut() else {
        return fallback;
    };
    let app_instance = object
        .remove("config_transaction_app_instance")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback.0);
    let batch_id = object
        .remove("config_transaction_batch_id")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback.1);
    let owner_process_id = object
        .remove("config_transaction_owner_process_id")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .unwrap_or(fallback.2);
    (app_instance, batch_id, owner_process_id)
}

/// Enqueue one replayable shared-config operation and drain the durable FIFO.
pub fn execute(
    config_dir: &Path,
    app_instance: &str,
    batch_id: &str,
    owner_process_id: u32,
    operation: &str,
    payload: Value,
) -> Result<Value, String> {
    if app_instance.is_empty() || batch_id.is_empty() || owner_process_id == 0 {
        return Err("config transaction requires app instance, batch id, and process id".into());
    }
    std::fs::create_dir_all(config_dir)
        .map_err(|error| format!("create config directory {}: {error}", config_dir.display()))?;
    let _lock = acquire_lock(config_dir)?;
    let mut journal = read_journal(config_dir)?;
    let mut history = read_history(config_dir)?;
    if let Some(existing) = history.iter().find(|row| row.batch_id == batch_id) {
        validate_identity(existing, operation, &payload)?;
        return result_from_terminal(existing);
    }
    if let Some(existing) = journal.batches.iter().find(|row| row.batch_id == batch_id) {
        validate_identity(existing, operation, &payload)?;
    } else {
        let envelope = ConfigTransactionEnvelope {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            batch_id: batch_id.to_string(),
            operation: operation.to_string(),
            app_instance: app_instance.to_string(),
            owner_process_id,
            status: ConfigTransactionStatus::Pending,
            payload,
            result: None,
            error: None,
            updated_unix_ms: unix_ms(),
        };
        journal.batches.push(envelope.clone());
        journal.updated_unix_ms = unix_ms();
        persist_journal(config_dir, &journal)?;
        checkpoint(operation, "after_enqueue", &envelope)?;
    }
    drain_locked(config_dir, &mut journal, &mut history)?;
    let terminal = history
        .iter()
        .find(|row| row.batch_id == batch_id)
        .ok_or_else(|| format!("config transaction {batch_id} did not reach history"))?;
    result_from_terminal(terminal)
}

/// Replay any pending config owner left by a terminated process.
pub fn recover(config_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| format!("create config directory {}: {error}", config_dir.display()))?;
    let _lock = acquire_lock(config_dir)?;
    let mut journal = read_journal(config_dir)?;
    let mut history = read_history(config_dir)?;
    drain_locked(config_dir, &mut journal, &mut history)
}

/// Recover pending writes and read the latest process-shared preferences.
pub fn load_preferences(config_dir: &Path) -> Result<Preferences, String> {
    recover(config_dir)?;
    let _lock = acquire_lock(config_dir)?;
    let path = config_dir.join("omegat.prefs.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let preferences = Preferences::default_in(config_dir.to_path_buf());
            preferences
                .save()
                .map_err(|error| format!("create shared preferences {}: {error}", path.display()))?;
            return Ok(preferences);
        }
        Err(error) => {
            return Err(format!(
                "read shared preferences {}: {error}",
                path.display()
            ))
        }
    };
    let mut preferences: Preferences = serde_json::from_str(&raw)
        .map_err(|error| format!("parse shared preferences {}: {error}", path.display()))?;
    preferences.config_dir = config_dir.to_path_buf();
    preferences.normalize();
    Ok(preferences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_patch_preserves_independent_nested_fields() {
        let base = json!({
            "locale": "en",
            "filter_options": {"text": {"preserve_spaces": "true", "encoding": "utf8"}},
        });
        let desired = json!({
            "locale": "fr",
            "filter_options": {"text": {"preserve_spaces": "true", "encoding": "utf8"}},
        });
        assert_eq!(preference_patch(&base, &desired), json!({"locale": "fr"}));

        let mut current = json!({
            "locale": "en",
            "filter_options": {"text": {"preserve_spaces": "true", "encoding": "utf16"}},
        });
        merge_patch(&mut current, &preference_patch(&base, &desired));
        assert_eq!(current["locale"], "fr");
        assert_eq!(current["filter_options"]["text"]["encoding"], "utf16");
    }

    #[test]
    fn config_fifo_merges_stale_field_updates_outside_project_journal() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let first = execute(
            &config,
            "electron-a",
            "batch-a",
            101,
            "prefs.patch",
            json!({"locale": "fr", "filter_options": {"text": {"preserve_spaces": "one"}}}),
        )
        .unwrap();
        assert_eq!(first["locale"], "fr");
        let second = execute(
            &config,
            "electron-b",
            "batch-b",
            202,
            "prefs.patch",
            json!({"theme": "dark", "filter_options": {"po": {"skip_header": "true"}}}),
        )
        .unwrap();
        assert_eq!(second["locale"], "fr");
        assert_eq!(second["theme"], "dark");
        assert_eq!(second["filter_options"]["text"]["preserve_spaces"], "one");
        assert_eq!(second["filter_options"]["po"]["skip_header"], "true");
        assert!(!active_path(&config).exists());
        let history = read_history(&config).unwrap();
        assert_eq!(
            history
                .iter()
                .map(|row| row.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec!["batch-a", "batch-b"]
        );
        assert!(!temp
            .path()
            .join(".repositories")
            .join("transactions")
            .exists());
    }

    #[test]
    fn terminal_batch_retry_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let payload = json!({"srx_path": "one.srx"});
        let first = execute(
            &config,
            "electron",
            "same",
            303,
            "prefs.patch",
            payload.clone(),
        )
        .unwrap();
        let second = execute(&config, "electron", "same", 303, "prefs.patch", payload).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            read_history(&config)
                .unwrap()
                .iter()
                .filter(|row| row.batch_id == "same")
                .count(),
            1
        );
    }
}
