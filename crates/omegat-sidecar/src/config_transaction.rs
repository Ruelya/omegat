// SPDX-License-Identifier: GPL-3.0-or-later

//! Process-shared transactions for configuration products.
//!
//! The active FIFO is config-scoped and protected by an OS lock. Terminal
//! results use `omegat-core`'s shared segmented-history implementation, the
//! same durable hot/manifest/segment/GC machinery used by project products.
//! Former v1/v2 sidecar-specific indexes are imported through a durable seed
//! before they are removed, so migration is restartable and old batch retries
//! retain their exact result.

use omegat_core::durable_fifo::{
    DurableFifoEntry, DurableFifoLayout, DurableFifoLock, DurableFifoState, LegacyFifoState,
};
use omegat_core::durable_transaction::{
    DurableTransactionLayout, DurableTransactionPhase, DurableTransactionRecord,
    DurableTransactionWorkflow,
};
use omegat_core::prefs::Preferences;
use omegat_core::segmented_history::{
    SegmentedHistory, SegmentedHistoryLayout, SegmentedHistoryOptions, SegmentedHistoryRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CONFIG_TRANSACTION_VERSION: u8 = 3;
const LEGACY_CONFIG_TRANSACTION_VERSION_1: u8 = 1;
const LEGACY_CONFIG_TRANSACTION_VERSION_2: u8 = 2;
const TRANSACTION_DIRECTORY: &str = "shared-config";
const CONFIG_HISTORY_LIMIT: usize = 64;
const CONFIG_DEDUPE_HOT_LIMIT: usize = 64;
const CONFIG_ARCHIVE_SEGMENT_LIMIT: usize = 64;
const CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT: usize = 64;
const CONFIG_ARCHIVE_BATCH_PREFIX_HEX: usize = 4;
const MIGRATION_SEED: &str = ".history-unified-migration.ndjson";
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

impl SegmentedHistoryRecord for ConfigTransactionEnvelope {
    fn history_partition(&self) -> &str {
        &self.batch_id
    }

    fn relocate(&mut self, _old_scope: &Path, new_scope: &Path) {
        self.version = CONFIG_TRANSACTION_VERSION;
        self.config_dir = normalized(new_scope);
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfigTransactionJournal {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    batches: Vec<ConfigTransactionEnvelope>,
    updated_unix_ms: u128,
}

type ConfigTransactionJournal = DurableFifoState<ConfigTransactionEnvelope>;

/// Former sidecar-private hot index. Kept only as a strict migration decoder.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfigDedupe {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    batches: BTreeMap<String, ConfigTransactionEnvelope>,
    #[serde(default)]
    order: Vec<String>,
    updated_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyArchiveDescriptor {
    id: u64,
    #[serde(default)]
    generation: u64,
    file: String,
    sha256: String,
    batch_count: usize,
    first_batch_id: String,
    last_batch_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyArchiveManifest {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    next_segment_id: u64,
    #[serde(default)]
    generation: u64,
    segments: Vec<LegacyArchiveDescriptor>,
    #[serde(default)]
    batch_index: BTreeMap<String, Vec<u64>>,
    #[serde(default)]
    batch_index_complete: bool,
    updated_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyArchiveSegment {
    version: u8,
    config_dir: PathBuf,
    id: u64,
    #[serde(default)]
    generation: u64,
    batches: Vec<ConfigTransactionEnvelope>,
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

#[cfg(test)]
fn active_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("active.json")
}

#[cfg(test)]
fn active_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("active.recovery.json")
}

fn history_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("history.ndjson")
}

fn migration_seed_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join(MIGRATION_SEED)
}

fn legacy_dedupe_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("dedupe.json")
}

fn legacy_dedupe_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("dedupe.recovery.json")
}

fn legacy_manifest_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("manifest.json")
}

fn legacy_manifest_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("manifest.recovery.json")
}

fn legacy_archive_dir(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("archive")
}

fn history_layout() -> SegmentedHistoryLayout {
    SegmentedHistoryLayout {
        recent_file: "history.ndjson".into(),
        hot_file: "history-hot.json".into(),
        hot_recovery_file: ".history-hot.recovery.json".into(),
        manifest_file: "history-manifest.json".into(),
        manifest_recovery_file: ".history-manifest.recovery.json".into(),
        archive_directory: "history-archive".into(),
    }
}

fn configured_limit(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn history_options() -> SegmentedHistoryOptions {
    let hot = configured_limit(
        "OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT",
        CONFIG_DEDUPE_HOT_LIMIT,
    );
    SegmentedHistoryOptions {
        recent_limit: configured_limit("OMEGAT_TEST_CONFIG_HISTORY_LIMIT", CONFIG_HISTORY_LIMIT)
            .min(hot),
        hot_limit: hot,
        segment_record_limit: configured_limit(
            "OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT",
            CONFIG_ARCHIVE_SEGMENT_LIMIT,
        ),
        generation_segment_limit: configured_limit(
            "OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT",
            CONFIG_ARCHIVE_COMPACTION_SEGMENT_LIMIT,
        )
        .max(2),
        generation_record_limit: configured_limit(
            "OMEGAT_TEST_CONFIG_ARCHIVE_COMPACTION_BATCH_LIMIT",
            CONFIG_ARCHIVE_SEGMENT_LIMIT,
        ),
        partition_prefix_hex: configured_limit(
            "OMEGAT_TEST_CONFIG_ARCHIVE_BATCH_PREFIX_HEX",
            CONFIG_ARCHIVE_BATCH_PREFIX_HEX,
        )
        .min(64),
    }
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

fn remove_durable(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove config transaction file {}: {error}",
            path.display()
        )),
    }
}

fn acquire_lock(config_dir: &Path) -> Result<DurableFifoLock, String> {
    let directory = transaction_dir(config_dir);
    DurableFifoLock::acquire(&directory, &fifo_layout().lock_file)
        .map_err(|error| format!("lock shared config {}: {error}", directory.display()))
}

fn cleanup_interrupted_candidates(config_dir: &Path) -> Result<(), String> {
    let directory = transaction_dir(config_dir);
    let durable_names = [
        "active.json",
        "active.recovery.json",
        "dedupe.json",
        "dedupe.recovery.json",
        "manifest.json",
        "manifest.recovery.json",
        MIGRATION_SEED,
    ];
    let mut removed = false;
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("read config transaction directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("read config transaction directory entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let candidate = durable_names
            .iter()
            .any(|target| name.starts_with(&format!(".{target}.")))
            || name.starts_with(".archive-segment.")
            || name.starts_with(".archive-gc.");
        if entry
            .file_type()
            .map_err(|error| format!("inspect config transaction candidate: {error}"))?
            .is_file()
            && name.ends_with(".tmp")
            && candidate
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
            .map_err(|error| format!("sync cleaned config transaction directory: {error}"))?;
    }
    Ok(())
}

fn supported_version(version: u8) -> bool {
    matches!(
        version,
        CONFIG_TRANSACTION_VERSION
            | LEGACY_CONFIG_TRANSACTION_VERSION_1
            | LEGACY_CONFIG_TRANSACTION_VERSION_2
    )
}

fn valid_envelope(envelope: &ConfigTransactionEnvelope, scope: &Path, pending: bool) -> bool {
    supported_version(envelope.version)
        && normalized(&envelope.config_dir) == normalized(scope)
        && !envelope.batch_id.is_empty()
        && !envelope.operation.is_empty()
        && !envelope.app_instance.is_empty()
        && envelope.owner_process_id != 0
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

fn rebase_envelope(
    mut envelope: ConfigTransactionEnvelope,
    config_dir: &Path,
) -> ConfigTransactionEnvelope {
    envelope.version = CONFIG_TRANSACTION_VERSION;
    envelope.config_dir = normalized(config_dir);
    envelope
}

impl DurableFifoEntry for ConfigTransactionEnvelope {
    fn durable_fifo_id(&self) -> &str {
        &self.batch_id
    }

    fn validate_for_scope(&self, scope: &Path) -> Result<(), String> {
        if valid_envelope(self, scope, true) || valid_envelope(self, scope, false) {
            Ok(())
        } else {
            Err(format!(
                "invalid config transaction {}",
                self.batch_id
            ))
        }
    }

    fn relocate(&mut self, _old_scope: &Path, new_scope: &Path) {
        *self = rebase_envelope(self.clone(), new_scope);
    }
}

impl DurableTransactionRecord for ConfigTransactionEnvelope {
    fn transaction_phase(&self) -> DurableTransactionPhase {
        match self.status {
            ConfigTransactionStatus::Pending => DurableTransactionPhase::Pending,
            ConfigTransactionStatus::Completed | ConfigTransactionStatus::Failed => {
                DurableTransactionPhase::Acknowledged
            }
        }
    }

    fn validate_history_for_scope(&self, scope: &Path) -> Result<(), String> {
        if valid_envelope(self, scope, false) {
            Ok(())
        } else {
            Err(format!(
                "invalid terminal config transaction {}",
                self.batch_id
            ))
        }
    }
}

fn fifo_layout() -> DurableFifoLayout {
    DurableFifoLayout {
        primary_file: "active.json".into(),
        recovery_file: "active.recovery.json".into(),
        lock_file: "operation.lock".into(),
        ..DurableFifoLayout::default()
    }
}

fn decode_legacy_active(
    bytes: &[u8],
) -> Result<Option<LegacyFifoState<ConfigTransactionEnvelope>>, String> {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if value.get("config_dir").is_none() || value.get("batches").is_none() {
        return Ok(None);
    }
    let journal: LegacyConfigTransactionJournal = serde_json::from_value(value)
        .map_err(|error| format!("config transaction active journal: {error}"))?;
    if !supported_version(journal.version) || journal.config_dir.as_os_str().is_empty() {
        return Err(format!(
            "unsupported config transaction active version {}",
            journal.version
        ));
    }
    if journal
        .batches
        .iter()
        .any(|batch| !valid_envelope(batch, &journal.config_dir, true))
    {
        return Err("invalid legacy config transaction active batch".into());
    }
    Ok(Some(LegacyFifoState {
        scope: journal.config_dir,
        revision: journal.revision,
        batches: journal.batches,
        updated_unix_ms: journal.updated_unix_ms,
    }))
}

fn read_journal(config_dir: &Path) -> Result<ConfigTransactionJournal, String> {
    Ok(open_workflow_with_options(config_dir, history_options())?.into_queue())
}

fn persist_journal(
    config_dir: &Path,
    journal: &mut ConfigTransactionJournal,
) -> Result<(), String> {
    let mut workflow = open_workflow_with_options(config_dir, history_options())?;
    *workflow.queue_mut() = journal.clone();
    if workflow.queue().batches.is_empty() {
        workflow.clear_queue()
    } else {
        workflow.persist_queue()
    }?;
    *journal = workflow.into_queue();
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_json_replica<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<(bool, Option<T>), String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok((true, serde_json::from_slice(&bytes).ok())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok((false, None)),
        Err(error) => Err(format!(
            "read legacy config state {}: {error}",
            path.display()
        )),
    }
}

fn valid_legacy_dedupe(index: &LegacyConfigDedupe) -> bool {
    matches!(
        index.version,
        LEGACY_CONFIG_TRANSACTION_VERSION_1 | LEGACY_CONFIG_TRANSACTION_VERSION_2
    ) && !index.config_dir.as_os_str().is_empty()
        && index.order.len() == index.batches.len()
        && index.order.iter().collect::<BTreeSet<_>>().len() == index.order.len()
        && index.order.iter().all(|batch_id| {
            index.batches.get(batch_id).is_some_and(|batch| {
                batch.batch_id == *batch_id
                    && batch.version == index.version
                    && valid_envelope(batch, &index.config_dir, false)
            })
        })
}

fn select_legacy_dedupe(config_dir: &Path) -> Result<Vec<LegacyConfigDedupe>, String> {
    let replicas = [
        read_json_replica::<LegacyConfigDedupe>(&legacy_dedupe_path(config_dir))?,
        read_json_replica::<LegacyConfigDedupe>(&legacy_dedupe_recovery_path(config_dir))?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, value)| value.as_ref())
        .filter(|value| valid_legacy_dedupe(value))
        .cloned()
        .collect::<Vec<_>>();
    if valid.is_empty() && replicas.iter().any(|(exists, _)| *exists) {
        return Err(format!(
            "both legacy config transaction dedupe replicas are invalid in {}",
            transaction_dir(config_dir).display()
        ));
    }
    let mut selected = Vec::new();
    for version in [
        LEGACY_CONFIG_TRANSACTION_VERSION_1,
        LEGACY_CONFIG_TRANSACTION_VERSION_2,
    ] {
        let mut candidates = valid
            .drain(..)
            .filter(|candidate| candidate.version == version)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.revision);
        if let Some(last) = candidates.last().cloned() {
            if candidates
                .iter()
                .any(|candidate| candidate.revision == last.revision && candidate != &last)
            {
                return Err(format!(
                    "legacy config transaction v{version} dedupe replicas disagree at revision {}",
                    last.revision
                ));
            }
            selected.push(last);
        }
        valid.extend(
            candidates
                .into_iter()
                .filter(|candidate| candidate.version != version),
        );
    }
    Ok(selected)
}

fn valid_legacy_manifest(manifest: &LegacyArchiveManifest) -> bool {
    manifest.version == LEGACY_CONFIG_TRANSACTION_VERSION_2
        && !manifest.config_dir.as_os_str().is_empty()
        && manifest.next_segment_id > 0
        && manifest
            .segments
            .iter()
            .all(|segment| segment.generation == manifest.generation)
        && manifest
            .segments
            .iter()
            .map(|segment| segment.id)
            .collect::<BTreeSet<_>>()
            .len()
            == manifest.segments.len()
        && manifest
            .segments
            .iter()
            .map(|segment| segment.file.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            == manifest.segments.len()
}

fn select_legacy_manifest(config_dir: &Path) -> Result<Option<LegacyArchiveManifest>, String> {
    let replicas = [
        read_json_replica::<LegacyArchiveManifest>(&legacy_manifest_path(config_dir))?,
        read_json_replica::<LegacyArchiveManifest>(&legacy_manifest_recovery_path(config_dir))?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, value)| value.as_ref())
        .filter(|value| valid_legacy_manifest(value))
        .cloned()
        .collect::<Vec<_>>();
    if valid.is_empty() && replicas.iter().any(|(exists, _)| *exists) {
        return Err(format!(
            "both legacy config transaction manifest replicas are invalid in {}",
            transaction_dir(config_dir).display()
        ));
    }
    valid.sort_by_key(|manifest| manifest.revision);
    let selected = valid.last().cloned();
    if let Some(selected) = &selected {
        if valid
            .iter()
            .any(|candidate| candidate.revision == selected.revision && candidate != selected)
        {
            return Err(format!(
                "legacy config transaction manifest replicas disagree at revision {}",
                selected.revision
            ));
        }
    }
    Ok(selected)
}

fn legacy_segment(
    config_dir: &Path,
    file: &str,
) -> Result<(LegacyArchiveDescriptor, Vec<ConfigTransactionEnvelope>), String> {
    if Path::new(file).file_name().and_then(|name| name.to_str()) != Some(file)
        || !file.starts_with("segment-")
        || !file.ends_with(".json")
    {
        return Err(format!("unsafe legacy config archive segment name {file}"));
    }
    let path = legacy_archive_dir(config_dir).join(file);
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "read legacy config archive segment {}: {error}",
            path.display()
        )
    })?;
    let segment: LegacyArchiveSegment = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse legacy config archive segment {file}: {error}"))?;
    if segment.version != LEGACY_CONFIG_TRANSACTION_VERSION_2
        || segment.config_dir.as_os_str().is_empty()
        || segment.id == 0
        || segment.batches.is_empty()
        || segment.batches.iter().any(|batch| {
            batch.version != LEGACY_CONFIG_TRANSACTION_VERSION_2
                || !valid_envelope(batch, &segment.config_dir, false)
        })
    {
        return Err(format!("invalid legacy config archive segment {file}"));
    }
    let digest = sha256(&bytes);
    let old_name = format!("segment-{:020}-{digest}.json", segment.id);
    let generated_name = format!(
        "segment-g{:020}-{:020}-{digest}.json",
        segment.generation, segment.id
    );
    if file != old_name && file != generated_name {
        return Err(format!(
            "legacy config archive segment filename digest mismatch: {file}"
        ));
    }
    let descriptor = LegacyArchiveDescriptor {
        id: segment.id,
        generation: segment.generation,
        file: file.into(),
        sha256: digest,
        batch_count: segment.batches.len(),
        first_batch_id: segment
            .batches
            .first()
            .expect("non-empty legacy segment")
            .batch_id
            .clone(),
        last_batch_id: segment
            .batches
            .last()
            .expect("non-empty legacy segment")
            .batch_id
            .clone(),
    };
    Ok((descriptor, segment.batches))
}

fn legacy_archive_files(config_dir: &Path) -> Result<Vec<String>, String> {
    let directory = legacy_archive_dir(config_dir);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read legacy config archive {}: {error}",
                directory.display()
            ))
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read legacy archive entry: {error}"))?;
        if entry
            .file_type()
            .map_err(|error| format!("inspect legacy archive entry: {error}"))?
            .is_file()
        {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.starts_with("segment-") && file.ends_with(".json") {
                files.push(file);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn legacy_archive_records(config_dir: &Path) -> Result<Vec<ConfigTransactionEnvelope>, String> {
    let manifest = select_legacy_manifest(config_dir)?;
    let files = legacy_archive_files(config_dir)?;
    let mut decoded = BTreeMap::new();
    for file in files {
        let (descriptor, rows) = legacy_segment(config_dir, &file)?;
        decoded.insert(file, (descriptor, rows));
    }
    let generation = match &manifest {
        Some(manifest) => manifest.generation,
        None if decoded.is_empty() => 0,
        None => {
            let generations = decoded
                .values()
                .map(|(descriptor, _)| descriptor.generation)
                .collect::<BTreeSet<_>>();
            if generations.len() != 1 {
                return Err(
                    "legacy config archive has multiple generations without a manifest".into(),
                );
            }
            *generations.iter().next().expect("one legacy generation")
        }
    };
    let mut selected = BTreeMap::<u64, (LegacyArchiveDescriptor, Vec<_>)>::new();
    if let Some(manifest) = manifest {
        for expected in manifest.segments {
            let Some((actual, rows)) = decoded.remove(&expected.file) else {
                return Err(format!(
                    "legacy config archive manifest references missing segment {}",
                    expected.file
                ));
            };
            if actual != expected {
                return Err(format!(
                    "legacy config archive manifest descriptor disagrees with {}",
                    expected.file
                ));
            }
            selected.insert(expected.id, (expected, rows));
        }
    }
    for (_, (descriptor, rows)) in decoded {
        if descriptor.generation != generation {
            continue;
        }
        match selected.get(&descriptor.id) {
            Some((existing, _)) if existing == &descriptor => {}
            Some(_) => {
                return Err(format!(
                    "conflicting immutable legacy config archive segment {}",
                    descriptor.id
                ))
            }
            None => {
                selected.insert(descriptor.id, (descriptor, rows));
            }
        }
    }
    Ok(selected.into_values().flat_map(|(_, rows)| rows).collect())
}

fn read_legacy_recent(config_dir: &Path) -> Result<Vec<ConfigTransactionEnvelope>, String> {
    let bytes = match std::fs::read(history_path(config_dir)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("read legacy config history: {error}")),
    };
    let mut rows = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
    {
        let Some(row) = serde_json::from_slice::<ConfigTransactionEnvelope>(line)
            .ok()
            .filter(|row| valid_envelope(row, &row.config_dir, false))
        else {
            // The old hot index and immutable archive are authoritative. A
            // damaged recent projection is deliberately ignored.
            continue;
        };
        rows.push(row);
    }
    Ok(rows)
}

fn insert_legacy_terminal(
    config_dir: &Path,
    rows: &mut Vec<ConfigTransactionEnvelope>,
    by_id: &mut BTreeMap<String, ConfigTransactionEnvelope>,
    row: ConfigTransactionEnvelope,
) -> Result<(), String> {
    let row = rebase_envelope(row, config_dir);
    match by_id.get(&row.batch_id) {
        Some(existing) if existing == &row => return Ok(()),
        Some(_) => return Err(terminal_disagreement(&row.batch_id)),
        None => {}
    }
    by_id.insert(row.batch_id.clone(), row.clone());
    rows.push(row);
    Ok(())
}

fn legacy_terminal_records(config_dir: &Path) -> Result<Vec<ConfigTransactionEnvelope>, String> {
    let mut rows = Vec::new();
    let mut by_id = BTreeMap::new();
    for row in legacy_archive_records(config_dir)? {
        insert_legacy_terminal(config_dir, &mut rows, &mut by_id, row)?;
    }
    for dedupe in select_legacy_dedupe(config_dir)? {
        for batch_id in dedupe.order {
            let row = dedupe
                .batches
                .get(&batch_id)
                .expect("validated legacy dedupe order")
                .clone();
            insert_legacy_terminal(config_dir, &mut rows, &mut by_id, row)?;
        }
    }
    for row in read_legacy_recent(config_dir)? {
        insert_legacy_terminal(config_dir, &mut rows, &mut by_id, row)?;
    }
    Ok(rows)
}

fn legacy_history_exists(config_dir: &Path) -> bool {
    [
        legacy_dedupe_path(config_dir),
        legacy_dedupe_recovery_path(config_dir),
        legacy_manifest_path(config_dir),
        legacy_manifest_recovery_path(config_dir),
        legacy_archive_dir(config_dir),
    ]
    .iter()
    .any(|path| path.exists())
}

fn remove_legacy_history(config_dir: &Path) -> Result<(), String> {
    for path in [
        legacy_dedupe_path(config_dir),
        legacy_dedupe_recovery_path(config_dir),
        legacy_manifest_path(config_dir),
        legacy_manifest_recovery_path(config_dir),
    ] {
        remove_durable(&path)?;
    }
    let archive = legacy_archive_dir(config_dir);
    if archive.exists() {
        std::fs::remove_dir_all(&archive).map_err(|error| {
            format!(
                "remove migrated config archive {}: {error}",
                archive.display()
            )
        })?;
        sync_parent(&archive)?;
    }
    Ok(())
}

fn config_workflow_layout() -> DurableTransactionLayout {
    DurableTransactionLayout {
        fifo: fifo_layout(),
        history: history_layout(),
        migration_seed_file: MIGRATION_SEED.into(),
    }
}

fn open_workflow_with_options(
    config_dir: &Path,
    options: SegmentedHistoryOptions,
) -> Result<DurableTransactionWorkflow<ConfigTransactionEnvelope>, String> {
    let directory = transaction_dir(config_dir);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("create config transaction directory: {error}"))?;
    cleanup_interrupted_candidates(config_dir)?;
    let had_legacy = legacy_history_exists(config_dir);
    let workflow = DurableTransactionWorkflow::open_with_legacy(
        &directory,
        config_dir,
        config_workflow_layout(),
        options,
        decode_legacy_active,
        || legacy_terminal_records(config_dir),
        &mut |owner, point| {
            if let Some(owner) = owner {
                checkpoint(&owner.operation, point, owner)?;
            }
            Ok(())
        },
    )
    .map_err(|error| format!("config transaction workflow: {error}"))?;
    if had_legacy {
        remove_legacy_history(config_dir)?;
    }
    Ok(workflow)
}

fn open_history_with_options(
    config_dir: &Path,
    _owner: Option<&ConfigTransactionEnvelope>,
    options: SegmentedHistoryOptions,
) -> Result<SegmentedHistory<ConfigTransactionEnvelope>, String> {
    Ok(open_workflow_with_options(config_dir, options)?.into_history())
}

fn open_history(
    config_dir: &Path,
    owner: Option<&ConfigTransactionEnvelope>,
) -> Result<SegmentedHistory<ConfigTransactionEnvelope>, String> {
    open_history_with_options(config_dir, owner, history_options())
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
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => {
            return Err(format!(
                "create config checkpoint {}: {error}",
                marker.display()
            ))
        }
    };
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

fn terminal_disagreement(batch_id: &str) -> String {
    format!("config transaction terminal result disagrees for batch {batch_id}")
}

fn find_terminal(
    workflow: &DurableTransactionWorkflow<ConfigTransactionEnvelope>,
    batch_id: &str,
) -> Result<Option<ConfigTransactionEnvelope>, String> {
    workflow.terminal_record(batch_id).map_err(|error| {
        if error.contains("terminal result disagrees") {
            terminal_disagreement(batch_id)
        } else {
            format!("read config transaction terminal: {error}")
        }
    })
}

fn drain_locked(
    config_dir: &Path,
    workflow: &mut DurableTransactionWorkflow<ConfigTransactionEnvelope>,
) -> Result<(), String> {
    while let Some(pending) = workflow.queue().batches.first().cloned() {
        if pending.transaction_phase().is_terminal() {
            workflow
                .compact_terminals(
                    |candidate| candidate.transaction_phase().is_terminal(),
                    &mut |_| Ok(()),
                    &mut |point| checkpoint(&pending.operation, point, &pending),
                )
                .map_err(|error| format!("recover terminal config transaction: {error}"))?;
            continue;
        }
        if let Some(terminal) = find_terminal(workflow, &pending.batch_id)? {
            validate_identity(&terminal, &pending.operation, &pending.payload)?;
            workflow.remove(&pending.batch_id);
            workflow.persist_or_clear_queue()?;
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
            workflow
                .acknowledge_head(
                    &pending.batch_id,
                    terminal,
                    |candidate| candidate.status == ConfigTransactionStatus::Pending,
                    &mut |_| Ok(()),
                    &mut |point| {
                        checkpoint(&pending.operation, point, &pending)?;
                        if point == "after_terminal_history_publish" {
                            checkpoint(&pending.operation, "after_history_append", &pending)?;
                        }
                        Ok(())
                    },
                )
                .map_err(|error| format!("complete config transaction: {error}"))?;
        }
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
    let mut workflow = open_workflow_with_options(config_dir, history_options())?;
    if let Some(existing) = find_terminal(&workflow, batch_id)? {
        validate_identity(&existing, operation, &payload)?;
        return result_from_terminal(&existing);
    }
    if let Some(existing) = workflow
        .queue()
        .batches
        .iter()
        .find(|row| row.batch_id == batch_id)
    {
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
        workflow.upsert(envelope.clone())?;
        workflow.persist_queue()?;
        checkpoint(operation, "after_enqueue", &envelope)?;
    }
    drain_locked(config_dir, &mut workflow)?;
    let terminal = find_terminal(&workflow, batch_id)?
        .ok_or_else(|| format!("config transaction {batch_id} did not reach history"))?;
    result_from_terminal(&terminal)
}

/// Replay any pending config owner left by a terminated process.
pub fn recover(config_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| format!("create config directory {}: {error}", config_dir.display()))?;
    let _lock = acquire_lock(config_dir)?;
    let mut workflow = open_workflow_with_options(config_dir, history_options())?;
    drain_locked(config_dir, &mut workflow)
}

/// Recover pending writes and read the latest process-shared preferences.
pub fn load_preferences(config_dir: &Path) -> Result<Preferences, String> {
    recover(config_dir)?;
    let _lock = acquire_lock(config_dir)?;
    let path = config_dir.join("omegat.prefs.json");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => {
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

    fn completed(
        config: &Path,
        batch_id: &str,
        payload: Value,
        result: Value,
    ) -> ConfigTransactionEnvelope {
        let mut row = pending(config, batch_id, payload);
        row.status = ConfigTransactionStatus::Completed;
        row.result = Some(result);
        row
    }

    fn small_options() -> SegmentedHistoryOptions {
        SegmentedHistoryOptions {
            recent_limit: 2,
            hot_limit: 2,
            segment_record_limit: 1,
            generation_segment_limit: 3,
            generation_record_limit: 16,
            partition_prefix_hex: 1,
        }
    }

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
    fn config_fifo_merges_stale_fields_and_uses_unified_history_layout() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        execute(
            &config,
            "electron-a",
            "batch-a",
            101,
            "prefs.patch",
            json!({"locale": "fr", "filter_options": {"text": {"preserve_spaces": "one"}}}),
        )
        .unwrap();
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
        let history = open_history(&config, None).unwrap();
        assert_eq!(
            history
                .recent()
                .iter()
                .map(|row| row.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec!["batch-a", "batch-b"]
        );
        assert!(transaction_dir(&config)
            .join("history-manifest.json")
            .is_file());
        assert!(!legacy_dedupe_path(&config).exists());
        assert!(!legacy_manifest_path(&config).exists());
        assert!(!temp
            .path()
            .join(".repositories")
            .join("transactions")
            .exists());
    }

    #[test]
    fn terminal_batch_retry_is_exactly_idempotent() {
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
        let product = std::fs::read(config.join("omegat.prefs.json")).unwrap();
        let second = execute(&config, "electron", "same", 303, "prefs.patch", payload).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            std::fs::read(config.join("omegat.prefs.json")).unwrap(),
            product
        );
        assert_eq!(
            open_history(&config, None)
                .unwrap()
                .records_for("same")
                .unwrap()
                .len(),
            1
        );
        let error = execute(
            &config,
            "electron",
            "same",
            303,
            "prefs.patch",
            json!({"srx_path": "other.srx"}),
        )
        .unwrap_err();
        assert!(error.contains("reused for a different operation or payload"));
    }

    #[test]
    fn terminal_queue_publish_recovers_without_replaying_config_product() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let payload = json!({"locale": "fr"});
        let terminal = completed(
            &config,
            "published-before-history",
            payload.clone(),
            json!({"exact": "retained"}),
        );
        let mut workflow =
            open_workflow_with_options(&config, history_options()).unwrap();
        workflow.upsert(terminal.clone()).unwrap();
        workflow.persist_queue().unwrap();
        drop(workflow);

        recover(&config).unwrap();
        assert!(!active_path(&config).exists());
        assert!(!config.join("omegat.prefs.json").exists());
        assert_eq!(
            open_history(&config, None)
                .unwrap()
                .records_for("published-before-history")
                .unwrap(),
            vec![terminal]
        );
        assert_eq!(
            execute(
                &config,
                "replacement",
                "published-before-history",
                404,
                "prefs.patch",
                payload,
            )
            .unwrap(),
            json!({"exact": "retained"})
        );
        assert!(!config.join("omegat.prefs.json").exists());
    }

    #[test]
    fn active_replica_repairs_one_copy_and_two_corrupt_copies_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(transaction_dir(&config)).unwrap();
        let mut journal = ConfigTransactionJournal::empty(&config);
        journal
            .batches
            .push(pending(&config, "repair", json!({"locale": "fr"})));
        persist_journal(&config, &mut journal).unwrap();
        let recovery_bytes = std::fs::read(active_recovery_path(&config)).unwrap();
        std::fs::write(active_path(&config), b"{").unwrap();
        assert_eq!(read_journal(&config).unwrap().batches[0].batch_id, "repair");
        assert_eq!(std::fs::read(active_path(&config)).unwrap(), recovery_bytes);

        let mut disagreement = read_journal(&config).unwrap();
        disagreement.batches[0].payload = json!({"locale": "de"});
        std::fs::write(
            active_path(&config),
            serde_json::to_vec_pretty(&disagreement).unwrap(),
        )
        .unwrap();
        let error = recover(&config).unwrap_err();
        assert!(error.contains("durable FIFO replicas disagree at revision"));
        assert!(!config.join("omegat.prefs.json").exists());
        std::fs::write(active_path(&config), &recovery_bytes).unwrap();

        std::fs::write(active_path(&config), b"{").unwrap();
        std::fs::write(active_recovery_path(&config), b"not-json").unwrap();
        let error = recover(&config).unwrap_err();
        assert!(error.contains("both durable FIFO replicas are invalid"));
        assert!(!config.join("omegat.prefs.json").exists());
    }

    #[test]
    fn old_v1_v2_indexes_migrate_restartably_into_shared_segment_store() {
        let temp = tempfile::tempdir().unwrap();
        let old_config = temp.path().join("config-before-move");
        std::fs::create_dir_all(transaction_dir(&old_config)).unwrap();
        let archived = completed(
            &old_config,
            "legacy-archived",
            json!({"theme": "legacy"}),
            json!({"exact": "archived"}),
        );
        let mut legacy_archived = archived.clone();
        legacy_archived.version = LEGACY_CONFIG_TRANSACTION_VERSION_2;
        let segment = LegacyArchiveSegment {
            version: LEGACY_CONFIG_TRANSACTION_VERSION_2,
            config_dir: normalized(&old_config),
            id: 1,
            generation: 0,
            batches: vec![legacy_archived.clone()],
        };
        let bytes = serde_json::to_vec_pretty(&segment).unwrap();
        let digest = sha256(&bytes);
        let file = format!("segment-g{:020}-{:020}-{digest}.json", 0, 1);
        std::fs::create_dir_all(legacy_archive_dir(&old_config)).unwrap();
        std::fs::write(legacy_archive_dir(&old_config).join(&file), &bytes).unwrap();
        let descriptor = LegacyArchiveDescriptor {
            id: 1,
            generation: 0,
            file,
            sha256: digest,
            batch_count: 1,
            first_batch_id: "legacy-archived".into(),
            last_batch_id: "legacy-archived".into(),
        };
        let manifest = LegacyArchiveManifest {
            version: LEGACY_CONFIG_TRANSACTION_VERSION_2,
            config_dir: normalized(&old_config),
            revision: 3,
            next_segment_id: 2,
            generation: 0,
            segments: vec![descriptor],
            batch_index: BTreeMap::new(),
            batch_index_complete: false,
            updated_unix_ms: unix_ms(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        std::fs::write(legacy_manifest_path(&old_config), &manifest_bytes).unwrap();
        std::fs::write(legacy_manifest_recovery_path(&old_config), &manifest_bytes).unwrap();
        let hot = completed(
            &old_config,
            "legacy-hot",
            json!({"locale": "fr"}),
            json!({"exact": "hot"}),
        );
        let mut legacy_hot = hot.clone();
        legacy_hot.version = LEGACY_CONFIG_TRANSACTION_VERSION_1;
        let dedupe = LegacyConfigDedupe {
            version: LEGACY_CONFIG_TRANSACTION_VERSION_1,
            config_dir: normalized(&old_config),
            revision: 4,
            batches: BTreeMap::from([("legacy-hot".into(), legacy_hot)]),
            order: vec!["legacy-hot".into()],
            updated_unix_ms: unix_ms(),
        };
        let dedupe_bytes = serde_json::to_vec_pretty(&dedupe).unwrap();
        std::fs::write(legacy_dedupe_path(&old_config), &dedupe_bytes).unwrap();
        std::fs::write(legacy_dedupe_recovery_path(&old_config), dedupe_bytes).unwrap();

        let new_config = temp.path().join("config-after-move");
        std::fs::rename(&old_config, &new_config).unwrap();
        let history = open_history_with_options(&new_config, None, small_options()).unwrap();
        assert_eq!(
            history.records_for("legacy-archived").unwrap()[0].result,
            archived.result
        );
        assert_eq!(
            history.records_for("legacy-hot").unwrap()[0].result,
            hot.result
        );
        assert!(!migration_seed_path(&new_config).exists());
        assert!(!legacy_archive_dir(&new_config).exists());
        assert!(!legacy_dedupe_path(&new_config).exists());
        assert!(!legacy_manifest_path(&new_config).exists());
        assert_eq!(
            execute(
                &new_config,
                "retry",
                "legacy-archived",
                42,
                "prefs.patch",
                json!({"theme": "legacy"}),
            )
            .unwrap(),
            json!({"exact": "archived"})
        );
        assert!(!new_config.join("omegat.prefs.json").exists());
    }

    #[test]
    fn unified_history_bounds_recent_handles_prefix_collisions_and_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        let mut history = open_history_with_options(&config, None, small_options()).unwrap();
        let mut ids_by_prefix = BTreeMap::<String, String>::new();
        let mut collision = None;
        for index in 0..128 {
            let id = format!("prefix-{index}");
            let prefix = sha256(id.as_bytes())[..1].to_string();
            if let Some(first) = ids_by_prefix.insert(prefix, id.clone()) {
                collision = Some((first, id));
                break;
            }
        }
        let (first, second) = collision.expect("one-hex prefix collision");
        let mut ids = vec![
            first.clone(),
            "middle-a".into(),
            "middle-b".into(),
            second.clone(),
        ];
        ids.dedup();
        for (index, id) in ids.iter().enumerate() {
            history
                .append(completed(
                    &config,
                    id,
                    json!({"theme": id}),
                    json!({"sequence": index}),
                ))
                .unwrap();
        }
        assert_eq!(history.recent().len(), 2);
        assert_eq!(history.records_for(&first).unwrap().len(), 1);
        assert_eq!(history.records_for(&second).unwrap().len(), 1);
        drop(history);

        let layout = history_layout();
        let directory = transaction_dir(&config);
        std::fs::write(directory.join(&layout.hot_file), b"{").unwrap();
        std::fs::write(directory.join(&layout.hot_recovery_file), b"bad").unwrap();
        let error = open_history_with_options(&config, None, small_options()).unwrap_err();
        assert!(error.contains("both segmented history hot replicas are invalid"));
    }
}
