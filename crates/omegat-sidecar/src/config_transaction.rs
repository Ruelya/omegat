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
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CONFIG_TRANSACTION_VERSION: u8 = 1;
const TRANSACTION_DIRECTORY: &str = "shared-config";
const CONFIG_HISTORY_LIMIT: usize = 64;
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigTransactionJournal {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    batches: Vec<ConfigTransactionEnvelope>,
    updated_unix_ms: u128,
}

impl ConfigTransactionJournal {
    fn empty(config_dir: &Path) -> Self {
        Self {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            revision: 0,
            batches: Vec::new(),
            updated_unix_ms: unix_ms(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigTransactionDedupe {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    batches: BTreeMap<String, ConfigTransactionEnvelope>,
    #[serde(default)]
    order: Vec<String>,
    updated_unix_ms: u128,
}

impl ConfigTransactionDedupe {
    fn empty(config_dir: &Path) -> Self {
        Self {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            revision: 0,
            batches: BTreeMap::new(),
            order: Vec::new(),
            updated_unix_ms: unix_ms(),
        }
    }
}

struct ConfigTransactionHistory {
    recent: Vec<ConfigTransactionEnvelope>,
    dedupe: ConfigTransactionDedupe,
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

fn active_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("active.recovery.json")
}

fn history_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("history.ndjson")
}

fn dedupe_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("dedupe.json")
}

fn dedupe_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("dedupe.recovery.json")
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

fn cleanup_interrupted_candidates(config_dir: &Path) -> Result<(), String> {
    let directory = transaction_dir(config_dir);
    let prefixes = [
        ".active.json.",
        ".active.recovery.json.",
        ".history.ndjson.",
        ".dedupe.json.",
        ".dedupe.recovery.json.",
    ];
    let mut removed = false;
    for entry in std::fs::read_dir(&directory).map_err(|error| {
        format!(
            "read config transaction directory {}: {error}",
            directory.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "read config transaction directory entry {}: {error}",
                directory.display()
            )
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry
            .file_type()
            .map_err(|error| format!("inspect config transaction candidate: {error}"))?
            .is_file()
            && prefixes.iter().any(|prefix| name.starts_with(prefix))
            && name.ends_with(".tmp")
        {
            std::fs::remove_file(entry.path()).map_err(|error| {
                format!(
                    "remove interrupted config transaction candidate {}: {error}",
                    entry.path().display()
                )
            })?;
            removed = true;
        }
    }
    if removed {
        File::open(&directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| {
                format!(
                    "sync cleaned config transaction directory {}: {error}",
                    directory.display()
                )
            })?;
    }
    Ok(())
}

fn valid_envelope(envelope: &ConfigTransactionEnvelope, config_dir: &Path, pending: bool) -> bool {
    envelope.version == CONFIG_TRANSACTION_VERSION
        && normalized(&envelope.config_dir) == normalized(config_dir)
        && !envelope.batch_id.is_empty()
        && !envelope.operation.is_empty()
        && (envelope.status == ConfigTransactionStatus::Pending) == pending
        && if pending {
            envelope.result.is_none() && envelope.error.is_none()
        } else {
            match envelope.status {
                ConfigTransactionStatus::Completed => {
                    envelope.result.is_some() && envelope.error.is_none()
                }
                ConfigTransactionStatus::Failed => {
                    envelope.result.is_none() && envelope.error.is_some()
                }
                ConfigTransactionStatus::Pending => false,
            }
        }
}

fn read_journal_replica(
    path: &Path,
    config_dir: &Path,
) -> Result<(bool, Option<ConfigTransactionJournal>), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((false, None)),
        Err(error) => {
            return Err(format!(
                "read config transaction journal replica {}: {error}",
                path.display()
            ))
        }
    };
    let journal = serde_json::from_slice::<ConfigTransactionJournal>(&bytes)
        .ok()
        .filter(|journal| {
            journal.version == CONFIG_TRANSACTION_VERSION
                && normalized(&journal.config_dir) == normalized(config_dir)
                && journal
                    .batches
                    .iter()
                    .all(|batch| valid_envelope(batch, config_dir, true))
        });
    Ok((true, journal))
}

fn remove_durable(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove config transaction file {}: {error}",
            path.display()
        )),
    }
}

fn write_journal_replicas(
    config_dir: &Path,
    journal: &ConfigTransactionJournal,
) -> Result<(), String> {
    if journal.batches.is_empty() {
        // The recovery copy disappears first. A crash between the removals can
        // only expose the older primary; its terminal batch is already in the
        // dedupe index and is therefore removed without replaying the product.
        remove_durable(&active_recovery_path(config_dir))?;
        return remove_durable(&active_path(config_dir));
    }
    let bytes = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("serialize config transaction journal: {error}"))?;
    for path in [active_recovery_path(config_dir), active_path(config_dir)] {
        omegat_core::durable_file::replace(&path, &bytes).map_err(|error| {
            format!(
                "publish config transaction journal replica {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn read_journal(config_dir: &Path) -> Result<ConfigTransactionJournal, String> {
    cleanup_interrupted_candidates(config_dir)?;
    let replicas = [
        (
            active_path(config_dir),
            read_journal_replica(&active_path(config_dir), config_dir)?,
        ),
        (
            active_recovery_path(config_dir),
            read_journal_replica(&active_recovery_path(config_dir), config_dir)?,
        ),
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, (_, journal))| journal.as_ref())
        .collect::<Vec<_>>();
    if valid.is_empty() {
        if replicas.iter().any(|(_, (exists, _))| *exists) {
            return Err(format!(
                "both config transaction journal replicas are invalid in {}",
                transaction_dir(config_dir).display()
            ));
        }
        return Ok(ConfigTransactionJournal::empty(config_dir));
    }
    valid.sort_by_key(|journal| journal.revision);
    let selected = (*valid.last().expect("non-empty journal replicas")).clone();
    if valid
        .iter()
        .any(|journal| journal.revision == selected.revision && **journal != selected)
    {
        return Err(format!(
            "config transaction journal replicas disagree at revision {}",
            selected.revision
        ));
    }
    let repair = replicas.iter().any(|(_, (_, journal))| {
        journal
            .as_ref()
            .map(|journal| journal != &selected)
            .unwrap_or(true)
    });
    if repair {
        write_journal_replicas(config_dir, &selected)?;
    }
    Ok(selected)
}

fn persist_journal(
    config_dir: &Path,
    journal: &mut ConfigTransactionJournal,
) -> Result<(), String> {
    journal.revision = journal.revision.saturating_add(1);
    journal.updated_unix_ms = unix_ms();
    write_journal_replicas(config_dir, journal)
}

fn read_dedupe_replica(
    path: &Path,
    config_dir: &Path,
) -> Result<(bool, Option<ConfigTransactionDedupe>), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((false, None)),
        Err(error) => {
            return Err(format!(
                "read config transaction dedupe replica {}: {error}",
                path.display()
            ))
        }
    };
    let index = serde_json::from_slice::<ConfigTransactionDedupe>(&bytes)
        .ok()
        .filter(|index| {
            index.version == CONFIG_TRANSACTION_VERSION
                && normalized(&index.config_dir) == normalized(config_dir)
                && index.batches.iter().all(|(batch_id, batch)| {
                    batch_id == &batch.batch_id && valid_envelope(batch, config_dir, false)
                })
                && index.order.len() == index.batches.len()
                && index
                    .order
                    .iter()
                    .all(|batch_id| index.batches.contains_key(batch_id))
                && index
                    .order
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == index.order.len()
        });
    Ok((true, index))
}

fn write_dedupe_replicas(
    config_dir: &Path,
    dedupe: &ConfigTransactionDedupe,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(dedupe)
        .map_err(|error| format!("serialize config transaction dedupe index: {error}"))?;
    for path in [dedupe_recovery_path(config_dir), dedupe_path(config_dir)] {
        omegat_core::durable_file::replace(&path, &bytes).map_err(|error| {
            format!(
                "publish config transaction dedupe replica {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn read_dedupe(config_dir: &Path) -> Result<ConfigTransactionDedupe, String> {
    let replicas = [
        (
            dedupe_path(config_dir),
            read_dedupe_replica(&dedupe_path(config_dir), config_dir)?,
        ),
        (
            dedupe_recovery_path(config_dir),
            read_dedupe_replica(&dedupe_recovery_path(config_dir), config_dir)?,
        ),
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, (_, index))| index.as_ref())
        .collect::<Vec<_>>();
    if valid.is_empty() {
        if replicas.iter().any(|(_, (exists, _))| *exists) {
            return Err(format!(
                "both config transaction dedupe replicas are invalid in {}",
                transaction_dir(config_dir).display()
            ));
        }
        return Ok(ConfigTransactionDedupe::empty(config_dir));
    }
    valid.sort_by_key(|index| index.revision);
    let selected = (*valid.last().expect("non-empty dedupe replicas")).clone();
    if valid
        .iter()
        .any(|index| index.revision == selected.revision && **index != selected)
    {
        return Err(format!(
            "config transaction dedupe replicas disagree at revision {}",
            selected.revision
        ));
    }
    let repair = replicas.iter().any(|(_, (_, index))| {
        index
            .as_ref()
            .map(|index| index != &selected)
            .unwrap_or(true)
    });
    if repair {
        write_dedupe_replicas(config_dir, &selected)?;
    }
    Ok(selected)
}

fn persist_dedupe(config_dir: &Path, dedupe: &mut ConfigTransactionDedupe) -> Result<(), String> {
    dedupe.revision = dedupe.revision.saturating_add(1);
    dedupe.updated_unix_ms = unix_ms();
    write_dedupe_replicas(config_dir, dedupe)
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

fn history_limit() -> usize {
    std::env::var("OMEGAT_TEST_CONFIG_HISTORY_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(CONFIG_HISTORY_LIMIT)
}

fn read_history(config_dir: &Path) -> Result<ConfigTransactionHistory, String> {
    let mut dedupe = read_dedupe(config_dir)?;
    let path = history_path(config_dir);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(format!(
                "read config transaction history {}: {error}",
                path.display()
            ))
        }
    };
    let mut damaged = !bytes.is_empty() && !bytes.ends_with(b"\n");
    let mut recent = Vec::<ConfigTransactionEnvelope>::new();
    let mut recent_by_id = BTreeMap::<String, ConfigTransactionEnvelope>::new();
    for line in bytes.split(|byte| *byte == b'\n').filter(|line| {
        line.iter()
            .any(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
    }) {
        let Some(envelope) = serde_json::from_slice::<ConfigTransactionEnvelope>(line)
            .ok()
            .filter(|envelope| valid_envelope(envelope, config_dir, false))
        else {
            damaged = true;
            continue;
        };
        if let Some(existing) = recent_by_id.get(&envelope.batch_id) {
            if existing != &envelope {
                return Err(format!(
                    "conflicting config transaction history rows for batch {}",
                    envelope.batch_id
                ));
            }
            damaged = true;
            continue;
        }
        recent_by_id.insert(envelope.batch_id.clone(), envelope.clone());
        recent.push(envelope);
    }

    let mut dedupe_changed = false;
    for envelope in &recent {
        match dedupe.batches.get(&envelope.batch_id) {
            Some(existing) if existing == envelope => {}
            Some(existing) => {
                validate_identity(existing, &envelope.operation, &envelope.payload)?;
                return Err(format!(
                    "config transaction terminal result disagrees for batch {}",
                    envelope.batch_id
                ));
            }
            None => {
                dedupe
                    .batches
                    .insert(envelope.batch_id.clone(), envelope.clone());
                dedupe.order.push(envelope.batch_id.clone());
                dedupe_changed = true;
            }
        }
    }
    if dedupe_changed {
        persist_dedupe(config_dir, &mut dedupe)?;
    }

    let limit = history_limit();
    let expected_recent = dedupe
        .order
        .iter()
        .rev()
        .take(limit)
        .rev()
        .map(|batch_id| {
            dedupe
                .batches
                .get(batch_id)
                .expect("validated dedupe order")
                .clone()
        })
        .collect::<Vec<_>>();
    if recent != expected_recent {
        recent = expected_recent;
        damaged = true;
    }
    if damaged {
        persist_history(config_dir, &recent)?;
    }
    Ok(ConfigTransactionHistory { recent, dedupe })
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
            "config transaction batch {} was reused for a different operation or payload",
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
    history: &mut ConfigTransactionHistory,
) -> Result<(), String> {
    while let Some(pending) = journal.batches.first().cloned() {
        if let Some(terminal) = history.dedupe.batches.get(&pending.batch_id) {
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
            history.recent.push(terminal.clone());
            persist_history(config_dir, &history.recent)?;
            checkpoint(&pending.operation, "after_history_append", &terminal)?;
            history
                .dedupe
                .batches
                .insert(terminal.batch_id.clone(), terminal.clone());
            history.dedupe.order.push(terminal.batch_id.clone());
            persist_dedupe(config_dir, &mut history.dedupe)?;
            if history.recent.len() > history_limit() {
                checkpoint(&pending.operation, "before_history_compaction", &terminal)?;
                history.recent = history
                    .recent
                    .split_off(history.recent.len() - history_limit());
                persist_history(config_dir, &history.recent)?;
                checkpoint(&pending.operation, "after_history_compaction", &terminal)?;
            }
        }
        journal.batches.remove(0);
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
    if let Some(existing) = history.dedupe.batches.get(batch_id) {
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
        persist_journal(config_dir, &mut journal)?;
        checkpoint(operation, "after_enqueue", &envelope)?;
    }
    drain_locked(config_dir, &mut journal, &mut history)?;
    let terminal = history
        .dedupe
        .batches
        .get(batch_id)
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
            preferences.save().map_err(|error| {
                format!("create shared preferences {}: {error}", path.display())
            })?;
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
                .recent
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
                .dedupe
                .batches
                .values()
                .filter(|row| row.batch_id == "same")
                .count(),
            1
        );
    }

    fn pending(config: &Path, batch_id: &str, payload: Value) -> ConfigTransactionEnvelope {
        ConfigTransactionEnvelope {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config),
            batch_id: batch_id.into(),
            operation: "prefs.patch".into(),
            app_instance: "electron-owner".into(),
            owner_process_id: 707,
            status: ConfigTransactionStatus::Pending,
            payload,
            result: None,
            error: None,
            updated_unix_ms: unix_ms(),
        }
    }

    #[test]
    fn active_recovery_replica_repairs_truncation_before_replay() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(transaction_dir(&config)).unwrap();
        let mut journal = ConfigTransactionJournal::empty(&config);
        journal
            .batches
            .push(pending(&config, "recover-active", json!({"locale": "fr"})));
        persist_journal(&config, &mut journal).unwrap();
        let recovery_bytes = std::fs::read(active_recovery_path(&config)).unwrap();
        std::fs::write(active_path(&config), b"{\"version\":").unwrap();

        let repaired = read_journal(&config).unwrap();
        assert_eq!(repaired.batches[0].batch_id, "recover-active");
        assert_eq!(std::fs::read(active_path(&config)).unwrap(), recovery_bytes);
        assert_eq!(
            std::fs::read(active_recovery_path(&config)).unwrap(),
            recovery_bytes
        );

        recover(&config).unwrap();
        assert!(!active_path(&config).exists());
        assert!(!active_recovery_path(&config).exists());
        assert_eq!(Preferences::load_or_default(&config).locale, "fr");
        let history = read_history(&config).unwrap();
        assert_eq!(history.dedupe.order, vec!["recover-active"]);
        assert_eq!(history.recent.len(), 1);
    }

    #[test]
    fn two_corrupt_active_replicas_stop_before_product_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(transaction_dir(&config)).unwrap();
        let mut journal = ConfigTransactionJournal::empty(&config);
        journal
            .batches
            .push(pending(&config, "reject-active", json!({"locale": "fr"})));
        persist_journal(&config, &mut journal).unwrap();
        std::fs::write(active_path(&config), b"{").unwrap();
        std::fs::write(active_recovery_path(&config), b"not-json").unwrap();

        let error = recover(&config).unwrap_err();
        assert!(error.contains("both config transaction journal replicas are invalid"));
        assert!(!config.join("omegat.prefs.json").exists());
        assert_eq!(std::fs::read(active_path(&config)).unwrap(), b"{");
        assert_eq!(
            std::fs::read(active_recovery_path(&config)).unwrap(),
            b"not-json"
        );
    }

    #[test]
    fn bounded_history_repairs_damage_and_keeps_exact_retry_result() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let first = execute(
            &config,
            "electron-a",
            "bounded-0",
            808,
            "prefs.patch",
            json!({"theme": "theme-0"}),
        )
        .unwrap();
        for index in 1..(CONFIG_HISTORY_LIMIT + 9) {
            execute(
                &config,
                "electron-b",
                &format!("bounded-{index}"),
                909,
                "prefs.patch",
                json!({"theme": format!("theme-{index}")}),
            )
            .unwrap();
        }

        let state = read_history(&config).unwrap();
        assert_eq!(state.recent.len(), CONFIG_HISTORY_LIMIT);
        assert_eq!(state.dedupe.batches.len(), CONFIG_HISTORY_LIMIT + 9);
        assert_eq!(state.dedupe.order.first().unwrap(), "bounded-0");
        assert_eq!(
            state.dedupe.order.last().unwrap(),
            &format!("bounded-{}", CONFIG_HISTORY_LIMIT + 8)
        );
        let product_before_retry = std::fs::read(config.join("omegat.prefs.json")).unwrap();
        assert_eq!(
            execute(
                &config,
                "electron-retry",
                "bounded-0",
                1001,
                "prefs.patch",
                json!({"theme": "theme-0"}),
            )
            .unwrap(),
            first
        );
        assert_eq!(
            std::fs::read(config.join("omegat.prefs.json")).unwrap(),
            product_before_retry
        );
        let conflict = execute(
            &config,
            "electron-conflict",
            "bounded-0",
            1002,
            "prefs.patch",
            json!({"theme": "conflict"}),
        )
        .unwrap_err();
        assert!(conflict.contains("reused for a different operation"));
        assert_eq!(
            std::fs::read(config.join("omegat.prefs.json")).unwrap(),
            product_before_retry
        );

        let history_path = history_path(&config);
        let mut lines = std::fs::read_to_string(&history_path)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines[3] = "{\"truncated\":".into();
        let mut damaged = lines.join("\n");
        damaged.push_str("\n{\"unterminated\":");
        std::fs::write(&history_path, damaged).unwrap();
        recover(&config).unwrap();

        let repaired = read_history(&config).unwrap();
        assert_eq!(repaired.recent.len(), CONFIG_HISTORY_LIMIT);
        assert_eq!(repaired.dedupe.batches.len(), CONFIG_HISTORY_LIMIT + 9);
        assert_eq!(
            std::fs::read_to_string(history_path)
                .unwrap()
                .lines()
                .map(|line| {
                    serde_json::from_str::<ConfigTransactionEnvelope>(line)
                        .unwrap()
                        .batch_id
                })
                .collect::<Vec<_>>(),
            repaired
                .dedupe
                .order
                .iter()
                .rev()
                .take(CONFIG_HISTORY_LIMIT)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            std::fs::read(dedupe_path(&config)).unwrap(),
            std::fs::read(dedupe_recovery_path(&config)).unwrap()
        );
        assert!(std::fs::read_dir(transaction_dir(&config))
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp")));
    }
}
