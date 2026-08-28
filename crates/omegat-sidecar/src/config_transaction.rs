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
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CONFIG_TRANSACTION_VERSION: u8 = 2;
const LEGACY_CONFIG_TRANSACTION_VERSION: u8 = 1;
const TRANSACTION_DIRECTORY: &str = "shared-config";
const CONFIG_HISTORY_LIMIT: usize = 64;
const CONFIG_DEDUPE_HOT_LIMIT: usize = 64;
const CONFIG_ARCHIVE_SEGMENT_LIMIT: usize = 64;
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigArchiveDescriptor {
    id: u64,
    file: String,
    sha256: String,
    batch_count: usize,
    first_batch_id: String,
    last_batch_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigArchiveManifest {
    version: u8,
    config_dir: PathBuf,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    next_segment_id: u64,
    segments: Vec<ConfigArchiveDescriptor>,
    updated_unix_ms: u128,
}

impl ConfigArchiveManifest {
    fn empty(config_dir: &Path) -> Self {
        Self {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            revision: 0,
            next_segment_id: 1,
            segments: Vec::new(),
            updated_unix_ms: unix_ms(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigArchiveSegment {
    version: u8,
    config_dir: PathBuf,
    id: u64,
    batches: Vec<ConfigTransactionEnvelope>,
}

struct ConfigTransactionHistory {
    recent: Vec<ConfigTransactionEnvelope>,
    dedupe: ConfigTransactionDedupe,
    manifest: ConfigArchiveManifest,
    archived: Vec<ConfigTransactionEnvelope>,
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

fn manifest_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("manifest.json")
}

fn manifest_recovery_path(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("manifest.recovery.json")
}

fn archive_dir(config_dir: &Path) -> PathBuf {
    transaction_dir(config_dir).join("archive")
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
        ".manifest.json.",
        ".manifest.recovery.json.",
        ".archive-segment.",
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

fn valid_envelope_version(
    envelope: &ConfigTransactionEnvelope,
    config_dir: &Path,
    pending: bool,
    version: u8,
) -> bool {
    envelope.version == version
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

fn valid_envelope(envelope: &ConfigTransactionEnvelope, config_dir: &Path, pending: bool) -> bool {
    valid_envelope_version(envelope, config_dir, pending, CONFIG_TRANSACTION_VERSION)
}

fn migrate_envelope(mut envelope: ConfigTransactionEnvelope) -> ConfigTransactionEnvelope {
    envelope.version = CONFIG_TRANSACTION_VERSION;
    envelope
}

fn read_journal_replica(
    path: &Path,
    config_dir: &Path,
) -> Result<(bool, Option<ConfigTransactionJournal>, bool), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((false, None, false));
        }
        Err(error) => {
            return Err(format!(
                "read config transaction journal replica {}: {error}",
                path.display()
            ))
        }
    };
    let Some(mut journal) = serde_json::from_slice::<ConfigTransactionJournal>(&bytes)
        .ok()
        .filter(|journal| {
            matches!(
                journal.version,
                CONFIG_TRANSACTION_VERSION | LEGACY_CONFIG_TRANSACTION_VERSION
            ) && normalized(&journal.config_dir) == normalized(config_dir)
                && journal
                    .batches
                    .iter()
                    .all(|batch| valid_envelope_version(batch, config_dir, true, journal.version))
        })
    else {
        return Ok((true, None, false));
    };
    let migrated = journal.version == LEGACY_CONFIG_TRANSACTION_VERSION;
    if migrated {
        journal.version = CONFIG_TRANSACTION_VERSION;
        journal.batches = journal.batches.into_iter().map(migrate_envelope).collect();
    }
    Ok((true, Some(journal), migrated))
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
        .filter_map(|(_, (_, journal, _))| journal.as_ref())
        .collect::<Vec<_>>();
    if valid.is_empty() {
        if replicas.iter().any(|(_, (exists, _, _))| *exists) {
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
    let repair = replicas.iter().any(|(_, (_, journal, migrated))| {
        if *migrated {
            return true;
        }
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

#[derive(Clone)]
enum ConfigDedupeReplica {
    Current(ConfigTransactionDedupe),
    Legacy(ConfigTransactionDedupe),
}

struct ConfigDedupeSources {
    current: ConfigTransactionDedupe,
    legacy: Option<ConfigTransactionDedupe>,
    needs_publish: bool,
}

fn valid_dedupe(index: &ConfigTransactionDedupe, config_dir: &Path) -> bool {
    matches!(
        index.version,
        CONFIG_TRANSACTION_VERSION | LEGACY_CONFIG_TRANSACTION_VERSION
    ) && normalized(&index.config_dir) == normalized(config_dir)
        && index.batches.iter().all(|(batch_id, batch)| {
            batch_id == &batch.batch_id
                && valid_envelope_version(batch, config_dir, false, index.version)
        })
        && index.order.len() == index.batches.len()
        && index
            .order
            .iter()
            .all(|batch_id| index.batches.contains_key(batch_id))
        && index.order.iter().collect::<BTreeSet<_>>().len() == index.order.len()
}

fn read_dedupe_replica(
    path: &Path,
    config_dir: &Path,
) -> Result<(bool, Option<ConfigDedupeReplica>), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((false, None)),
        Err(error) => {
            return Err(format!(
                "read config transaction dedupe replica {}: {error}",
                path.display()
            ))
        }
    };
    let index = serde_json::from_slice::<ConfigTransactionDedupe>(&bytes)
        .ok()
        .filter(|index| valid_dedupe(index, config_dir))
        .map(|index| {
            if index.version == CONFIG_TRANSACTION_VERSION {
                ConfigDedupeReplica::Current(index)
            } else {
                ConfigDedupeReplica::Legacy(index)
            }
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

fn persist_dedupe(config_dir: &Path, dedupe: &mut ConfigTransactionDedupe) -> Result<(), String> {
    dedupe.version = CONFIG_TRANSACTION_VERSION;
    dedupe.config_dir = normalized(config_dir);
    dedupe.revision = dedupe.revision.saturating_add(1);
    dedupe.updated_unix_ms = unix_ms();
    write_dedupe_replicas(config_dir, dedupe)
}

fn select_dedupe(
    mut candidates: Vec<ConfigTransactionDedupe>,
    label: &str,
) -> Result<Option<ConfigTransactionDedupe>, String> {
    candidates.sort_by_key(|index| index.revision);
    let Some(selected) = candidates.last().cloned() else {
        return Ok(None);
    };
    if candidates
        .iter()
        .any(|index| index.revision == selected.revision && index != &selected)
    {
        return Err(format!(
            "config transaction {label} dedupe replicas disagree at revision {}",
            selected.revision
        ));
    }
    Ok(Some(selected))
}

fn read_dedupe_sources(config_dir: &Path) -> Result<ConfigDedupeSources, String> {
    let replicas = [
        read_dedupe_replica(&dedupe_path(config_dir), config_dir)?,
        read_dedupe_replica(&dedupe_recovery_path(config_dir), config_dir)?,
    ];
    let mut current = Vec::new();
    let mut legacy = Vec::new();
    for (_, replica) in &replicas {
        match replica {
            Some(ConfigDedupeReplica::Current(index)) => current.push(index.clone()),
            Some(ConfigDedupeReplica::Legacy(index)) => legacy.push(index.clone()),
            None => {}
        }
    }
    if current.is_empty() && legacy.is_empty() && replicas.iter().any(|(exists, _)| *exists) {
        return Err(format!(
            "both config transaction dedupe replicas are invalid in {}",
            transaction_dir(config_dir).display()
        ));
    }
    let selected_current = select_dedupe(current, "v2")?;
    let selected_legacy = select_dedupe(legacy, "v1")?;
    let needs_publish = replicas.iter().any(|(_, replica)| match replica {
        Some(ConfigDedupeReplica::Current(index)) => {
            selected_current.as_ref() != Some(index) || selected_legacy.is_some()
        }
        Some(ConfigDedupeReplica::Legacy(_)) | None => true,
    });
    Ok(ConfigDedupeSources {
        current: selected_current.unwrap_or_else(|| ConfigTransactionDedupe::empty(config_dir)),
        legacy: selected_legacy,
        needs_publish,
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_manifest_replicas(
    config_dir: &Path,
    manifest: &ConfigArchiveManifest,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("serialize config transaction archive manifest: {error}"))?;
    for path in [
        manifest_recovery_path(config_dir),
        manifest_path(config_dir),
    ] {
        omegat_core::durable_file::replace(&path, &bytes).map_err(|error| {
            format!(
                "publish config transaction archive manifest replica {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn persist_manifest(config_dir: &Path, manifest: &mut ConfigArchiveManifest) -> Result<(), String> {
    manifest.version = CONFIG_TRANSACTION_VERSION;
    manifest.config_dir = normalized(config_dir);
    manifest.revision = manifest.revision.saturating_add(1);
    manifest.updated_unix_ms = unix_ms();
    write_manifest_replicas(config_dir, manifest)
}

fn read_manifest_replica(
    path: &Path,
    config_dir: &Path,
) -> Result<(bool, Option<ConfigArchiveManifest>), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((false, None)),
        Err(error) => {
            return Err(format!(
                "read config transaction archive manifest replica {}: {error}",
                path.display()
            ))
        }
    };
    let manifest = serde_json::from_slice::<ConfigArchiveManifest>(&bytes)
        .ok()
        .filter(|manifest| {
            manifest.version == CONFIG_TRANSACTION_VERSION
                && normalized(&manifest.config_dir) == normalized(config_dir)
                && manifest.next_segment_id > 0
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
        });
    Ok((true, manifest))
}

fn archive_descriptor(
    config_dir: &Path,
    file: String,
    bytes: &[u8],
) -> Result<(ConfigArchiveDescriptor, ConfigArchiveSegment), String> {
    if Path::new(&file).file_name().and_then(|name| name.to_str()) != Some(file.as_str())
        || !file.starts_with("segment-")
        || !file.ends_with(".json")
    {
        return Err(format!("unsafe config archive segment name {file}"));
    }
    let segment: ConfigArchiveSegment = serde_json::from_slice(bytes)
        .map_err(|error| format!("parse config archive segment {file}: {error}"))?;
    if segment.version != CONFIG_TRANSACTION_VERSION
        || normalized(&segment.config_dir) != normalized(config_dir)
        || segment.id == 0
        || segment.batches.is_empty()
        || segment
            .batches
            .iter()
            .any(|batch| !valid_envelope(batch, config_dir, false))
        || segment
            .batches
            .iter()
            .map(|batch| batch.batch_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != segment.batches.len()
    {
        return Err(format!("invalid config archive segment {file}"));
    }
    let digest = sha256(bytes);
    let expected_name = format!("segment-{:020}-{digest}.json", segment.id);
    if file != expected_name {
        return Err(format!(
            "config archive segment filename digest mismatch: {file}"
        ));
    }
    let descriptor = ConfigArchiveDescriptor {
        id: segment.id,
        file,
        sha256: digest,
        batch_count: segment.batches.len(),
        first_batch_id: segment
            .batches
            .first()
            .expect("non-empty archive")
            .batch_id
            .clone(),
        last_batch_id: segment
            .batches
            .last()
            .expect("non-empty archive")
            .batch_id
            .clone(),
    };
    Ok((descriptor, segment))
}

fn read_archive_descriptor(
    config_dir: &Path,
    expected: &ConfigArchiveDescriptor,
) -> Result<ConfigArchiveSegment, String> {
    let path = archive_dir(config_dir).join(&expected.file);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("read config archive segment {}: {error}", path.display()))?;
    let (actual, segment) = archive_descriptor(config_dir, expected.file.clone(), &bytes)?;
    if &actual != expected {
        return Err(format!(
            "config archive manifest descriptor disagrees with {}",
            path.display()
        ));
    }
    Ok(segment)
}

fn discover_archive_segments(
    config_dir: &Path,
) -> Result<Vec<(ConfigArchiveDescriptor, ConfigArchiveSegment)>, String> {
    let directory = archive_dir(config_dir);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read config archive directory {}: {error}",
                directory.display()
            ))
        }
    };
    let mut discovered = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read config archive directory entry {}: {error}",
                directory.display()
            )
        })?;
        if !entry
            .file_type()
            .map_err(|error| format!("inspect config archive entry: {error}"))?
            .is_file()
        {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        if !file.starts_with("segment-") || !file.ends_with(".json") {
            continue;
        }
        let bytes = std::fs::read(entry.path()).map_err(|error| {
            format!(
                "read discovered config archive segment {}: {error}",
                entry.path().display()
            )
        })?;
        discovered.push(archive_descriptor(config_dir, file, &bytes)?);
    }
    discovered.sort_by_key(|(descriptor, _)| descriptor.id);
    if discovered
        .iter()
        .map(|(descriptor, _)| descriptor.id)
        .collect::<BTreeSet<_>>()
        .len()
        != discovered.len()
    {
        return Err("duplicate config archive segment id".into());
    }
    Ok(discovered)
}

fn read_manifest(config_dir: &Path) -> Result<ConfigArchiveManifest, String> {
    let replicas = [
        read_manifest_replica(&manifest_path(config_dir), config_dir)?,
        read_manifest_replica(&manifest_recovery_path(config_dir), config_dir)?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, manifest)| manifest.as_ref())
        .collect::<Vec<_>>();
    if valid.is_empty() && replicas.iter().any(|(exists, _)| *exists) {
        return Err(format!(
            "both config transaction archive manifest replicas are invalid in {}",
            transaction_dir(config_dir).display()
        ));
    }
    valid.sort_by_key(|manifest| manifest.revision);
    let mut manifest = valid
        .last()
        .map(|manifest| (*manifest).clone())
        .unwrap_or_else(|| ConfigArchiveManifest::empty(config_dir));
    if valid
        .iter()
        .any(|candidate| candidate.revision == manifest.revision && **candidate != manifest)
    {
        return Err(format!(
            "config transaction archive manifest replicas disagree at revision {}",
            manifest.revision
        ));
    }

    for descriptor in &manifest.segments {
        read_archive_descriptor(config_dir, descriptor)?;
    }
    let discovered = discover_archive_segments(config_dir)?;
    let mut changed = valid.is_empty()
        || replicas.iter().any(|(_, candidate)| {
            candidate
                .as_ref()
                .map(|candidate| candidate != &manifest)
                .unwrap_or(true)
        });
    for (descriptor, _) in discovered {
        match manifest
            .segments
            .iter()
            .find(|candidate| candidate.id == descriptor.id)
        {
            Some(existing) if existing == &descriptor => {}
            Some(_) => {
                return Err(format!(
                    "conflicting immutable config archive segment {}",
                    descriptor.id
                ))
            }
            None => {
                manifest.segments.push(descriptor);
                changed = true;
            }
        }
    }
    manifest.segments.sort_by_key(|descriptor| descriptor.id);
    manifest.next_segment_id = manifest
        .segments
        .last()
        .map(|descriptor| descriptor.id.saturating_add(1))
        .unwrap_or(1);
    if changed {
        persist_manifest(config_dir, &mut manifest)?;
    }
    Ok(manifest)
}

fn publish_archive_segment(
    config_dir: &Path,
    manifest: &mut ConfigArchiveManifest,
    batches: &[ConfigTransactionEnvelope],
) -> Result<ConfigArchiveDescriptor, String> {
    let id = manifest.next_segment_id.max(1);
    let segment = ConfigArchiveSegment {
        version: CONFIG_TRANSACTION_VERSION,
        config_dir: normalized(config_dir),
        id,
        batches: batches.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&segment)
        .map_err(|error| format!("serialize config archive segment: {error}"))?;
    let digest = sha256(&bytes);
    let file = format!("segment-{id:020}-{digest}.json");
    let directory = archive_dir(config_dir);
    std::fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "create config archive directory {}: {error}",
            directory.display()
        )
    })?;
    sync_parent(&directory)?;
    let destination = directory.join(&file);
    if destination.exists() {
        let existing = std::fs::read(&destination).map_err(|error| {
            format!(
                "read existing config archive segment {}: {error}",
                destination.display()
            )
        })?;
        if existing != bytes {
            return Err(format!(
                "immutable config archive segment already exists with different bytes: {}",
                destination.display()
            ));
        }
    } else {
        let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = transaction_dir(config_dir).join(format!(
            ".archive-segment.{}.{sequence}.tmp",
            std::process::id()
        ));
        let write_result = (|| -> Result<(), String> {
            let mut candidate = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|error| {
                    format!(
                        "create config archive candidate {}: {error}",
                        temporary.display()
                    )
                })?;
            candidate.write_all(&bytes).map_err(|error| {
                format!(
                    "write config archive candidate {}: {error}",
                    temporary.display()
                )
            })?;
            checkpoint(
                &batches[0].operation,
                "after_archive_candidate_write",
                &batches[0],
            )?;
            candidate.sync_all().map_err(|error| {
                format!(
                    "sync config archive candidate {}: {error}",
                    temporary.display()
                )
            })?;
            checkpoint(
                &batches[0].operation,
                "after_archive_candidate_fsync",
                &batches[0],
            )?;
            std::fs::rename(&temporary, &destination).map_err(|error| {
                format!(
                    "publish immutable config archive segment {}: {error}",
                    destination.display()
                )
            })?;
            checkpoint(&batches[0].operation, "after_archive_rename", &batches[0])?;
            File::open(&directory)
                .and_then(|archive| archive.sync_all())
                .map_err(|error| {
                    format!(
                        "sync config archive directory {}: {error}",
                        directory.display()
                    )
                })?;
            checkpoint(
                &batches[0].operation,
                "after_archive_parent_fsync",
                &batches[0],
            )
        })();
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    }
    let (descriptor, _) = archive_descriptor(config_dir, file, &bytes)?;
    manifest.segments.push(descriptor.clone());
    manifest.segments.sort_by_key(|candidate| candidate.id);
    manifest.next_segment_id = id.saturating_add(1);
    persist_manifest(config_dir, manifest)?;
    Ok(descriptor)
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

fn configured_limit(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn history_limit() -> usize {
    configured_limit("OMEGAT_TEST_CONFIG_HISTORY_LIMIT", CONFIG_HISTORY_LIMIT)
}

fn dedupe_hot_limit() -> usize {
    configured_limit(
        "OMEGAT_TEST_CONFIG_DEDUPE_HOT_LIMIT",
        CONFIG_DEDUPE_HOT_LIMIT,
    )
}

fn archive_segment_limit() -> usize {
    configured_limit(
        "OMEGAT_TEST_CONFIG_ARCHIVE_SEGMENT_LIMIT",
        CONFIG_ARCHIVE_SEGMENT_LIMIT,
    )
}

fn terminal_disagreement(batch_id: &str) -> String {
    format!("config transaction terminal result disagrees for batch {batch_id}")
}

fn insert_unarchived(
    archived: &BTreeMap<String, ConfigTransactionEnvelope>,
    batches: &mut BTreeMap<String, ConfigTransactionEnvelope>,
    order: &mut Vec<String>,
    envelope: ConfigTransactionEnvelope,
) -> Result<(), String> {
    if let Some(existing) = archived.get(&envelope.batch_id) {
        if existing != &envelope {
            return Err(terminal_disagreement(&envelope.batch_id));
        }
        return Ok(());
    }
    match batches.get(&envelope.batch_id) {
        Some(existing) if existing == &envelope => Ok(()),
        Some(_) => Err(terminal_disagreement(&envelope.batch_id)),
        None => {
            order.push(envelope.batch_id.clone());
            batches.insert(envelope.batch_id.clone(), envelope);
            Ok(())
        }
    }
}

fn canonical_recent(history: &ConfigTransactionHistory) -> Vec<ConfigTransactionEnvelope> {
    history
        .archived
        .iter()
        .chain(history.dedupe.order.iter().map(|batch_id| {
            history
                .dedupe
                .batches
                .get(batch_id)
                .expect("validated hot dedupe order")
        }))
        .rev()
        .take(history_limit())
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn compact_history_storage(
    config_dir: &Path,
    history: &mut ConfigTransactionHistory,
    emit_checkpoints: bool,
) -> Result<bool, String> {
    let archive_count = history
        .dedupe
        .order
        .len()
        .saturating_sub(dedupe_hot_limit());
    let history_needs_compaction = history.recent.len() > history_limit();
    if archive_count == 0 && !history_needs_compaction {
        return Ok(false);
    }
    let checkpoint_envelope = history
        .dedupe
        .order
        .last()
        .and_then(|batch_id| history.dedupe.batches.get(batch_id))
        .cloned()
        .or_else(|| history.recent.last().cloned());
    if emit_checkpoints {
        if let Some(envelope) = &checkpoint_envelope {
            checkpoint(&envelope.operation, "before_history_compaction", envelope)?;
        }
    }

    if archive_count > 0 {
        let to_archive = history.dedupe.order[..archive_count]
            .iter()
            .map(|batch_id| {
                history
                    .dedupe
                    .batches
                    .get(batch_id)
                    .expect("validated hot dedupe order")
                    .clone()
            })
            .collect::<Vec<_>>();
        for chunk in to_archive.chunks(archive_segment_limit()) {
            publish_archive_segment(config_dir, &mut history.manifest, chunk)?;
        }
        for envelope in &to_archive {
            history.dedupe.batches.remove(&envelope.batch_id);
        }
        history.dedupe.order.drain(..archive_count);
        history.archived.extend(to_archive);
        persist_dedupe(config_dir, &mut history.dedupe)?;
    }

    let expected_recent = canonical_recent(history);
    if history.recent != expected_recent {
        history.recent = expected_recent;
        persist_history(config_dir, &history.recent)?;
    }
    if emit_checkpoints {
        if let Some(envelope) = &checkpoint_envelope {
            checkpoint(&envelope.operation, "after_history_compaction", envelope)?;
        }
    }
    Ok(true)
}

fn read_history(config_dir: &Path) -> Result<ConfigTransactionHistory, String> {
    cleanup_interrupted_candidates(config_dir)?;
    let manifest = read_manifest(config_dir)?;
    let mut archived = Vec::new();
    let mut archived_by_id = BTreeMap::new();
    for descriptor in &manifest.segments {
        let segment = read_archive_descriptor(config_dir, descriptor)?;
        for envelope in segment.batches {
            match archived_by_id.get(&envelope.batch_id) {
                Some(existing) if existing == &envelope => {
                    return Err(format!(
                        "config transaction batch {} occurs in multiple archive segments",
                        envelope.batch_id
                    ))
                }
                Some(_) => return Err(terminal_disagreement(&envelope.batch_id)),
                None => {
                    archived_by_id.insert(envelope.batch_id.clone(), envelope.clone());
                    archived.push(envelope);
                }
            }
        }
    }

    let path = history_path(config_dir);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(format!(
                "read config transaction history {}: {error}",
                path.display()
            ))
        }
    };
    let mut damaged = !bytes.is_empty() && !bytes.ends_with(b"\n");
    let mut disk_recent = Vec::new();
    let mut disk_recent_by_id = BTreeMap::<String, ConfigTransactionEnvelope>::new();
    for line in bytes.split(|byte| *byte == b'\n').filter(|line| {
        line.iter()
            .any(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
    }) {
        let Some(mut envelope) = serde_json::from_slice::<ConfigTransactionEnvelope>(line)
            .ok()
            .filter(|envelope| {
                matches!(
                    envelope.version,
                    CONFIG_TRANSACTION_VERSION | LEGACY_CONFIG_TRANSACTION_VERSION
                ) && valid_envelope_version(envelope, config_dir, false, envelope.version)
            })
        else {
            damaged = true;
            continue;
        };
        if envelope.version == LEGACY_CONFIG_TRANSACTION_VERSION {
            envelope = migrate_envelope(envelope);
            damaged = true;
        }
        if let Some(existing) = disk_recent_by_id.get(&envelope.batch_id) {
            if existing != &envelope {
                return Err(format!(
                    "conflicting config transaction history rows for batch {}",
                    envelope.batch_id
                ));
            }
            damaged = true;
            continue;
        }
        disk_recent_by_id.insert(envelope.batch_id.clone(), envelope.clone());
        disk_recent.push(envelope);
    }

    let ConfigDedupeSources {
        current,
        legacy,
        needs_publish,
    } = read_dedupe_sources(config_dir)?;
    let mut batches = BTreeMap::new();
    let mut order = Vec::new();
    if let Some(legacy) = legacy {
        for batch_id in &legacy.order {
            let envelope = migrate_envelope(
                legacy
                    .batches
                    .get(batch_id)
                    .expect("validated legacy dedupe order")
                    .clone(),
            );
            insert_unarchived(&archived_by_id, &mut batches, &mut order, envelope)?;
        }
    }
    for batch_id in &current.order {
        let envelope = current
            .batches
            .get(batch_id)
            .expect("validated current dedupe order")
            .clone();
        insert_unarchived(&archived_by_id, &mut batches, &mut order, envelope)?;
    }
    for envelope in &disk_recent {
        insert_unarchived(&archived_by_id, &mut batches, &mut order, envelope.clone())?;
    }
    let dedupe_needs_publish =
        needs_publish || batches != current.batches || order != current.order;

    let mut state = ConfigTransactionHistory {
        recent: disk_recent,
        dedupe: ConfigTransactionDedupe {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config_dir),
            revision: current.revision,
            batches,
            order,
            updated_unix_ms: current.updated_unix_ms,
        },
        manifest,
        archived,
    };
    compact_history_storage(config_dir, &mut state, false)?;
    if dedupe_needs_publish {
        persist_dedupe(config_dir, &mut state.dedupe)?;
    }
    let expected_recent = canonical_recent(&state);
    if damaged || state.recent != expected_recent {
        state.recent = expected_recent;
        persist_history(config_dir, &state.recent)?;
    }
    Ok(state)
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

fn find_terminal<'a>(
    history: &'a ConfigTransactionHistory,
    batch_id: &str,
) -> Option<&'a ConfigTransactionEnvelope> {
    history.dedupe.batches.get(batch_id).or_else(|| {
        history
            .archived
            .iter()
            .find(|envelope| envelope.batch_id == batch_id)
    })
}

fn drain_locked(
    config_dir: &Path,
    journal: &mut ConfigTransactionJournal,
    history: &mut ConfigTransactionHistory,
) -> Result<(), String> {
    while let Some(pending) = journal.batches.first().cloned() {
        if let Some(terminal) = find_terminal(history, &pending.batch_id) {
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
            compact_history_storage(config_dir, history, true)?;
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
    if let Some(existing) = find_terminal(&history, batch_id) {
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
    let terminal = find_terminal(&history, batch_id)
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
        assert_eq!(state.dedupe.batches.len(), CONFIG_DEDUPE_HOT_LIMIT);
        assert_eq!(state.archived.len(), 9);
        assert_eq!(state.dedupe.order.first().unwrap(), "bounded-9");
        assert_eq!(
            state.dedupe.order.last().unwrap(),
            &format!("bounded-{}", CONFIG_HISTORY_LIMIT + 8)
        );
        assert_eq!(state.manifest.segments.len(), 9);
        assert_eq!(
            std::fs::read(manifest_path(&config)).unwrap(),
            std::fs::read(manifest_recovery_path(&config)).unwrap()
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
        assert_eq!(repaired.dedupe.batches.len(), CONFIG_DEDUPE_HOT_LIMIT);
        assert_eq!(repaired.archived.len(), 9);
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
                .recent
                .iter()
                .map(|row| row.batch_id.clone())
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

    fn completed(
        config: &Path,
        batch_id: &str,
        payload: Value,
        result: Value,
    ) -> ConfigTransactionEnvelope {
        ConfigTransactionEnvelope {
            version: CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(config),
            batch_id: batch_id.into(),
            operation: "prefs.patch".into(),
            app_instance: "electron-owner".into(),
            owner_process_id: 808,
            status: ConfigTransactionStatus::Completed,
            payload,
            result: Some(result),
            error: None,
            updated_unix_ms: unix_ms(),
        }
    }

    fn as_legacy(mut envelope: ConfigTransactionEnvelope) -> ConfigTransactionEnvelope {
        envelope.version = LEGACY_CONFIG_TRANSACTION_VERSION;
        envelope
    }

    #[test]
    fn migrates_v1_active_and_history_when_the_legacy_index_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(transaction_dir(&config)).unwrap();
        let legacy_terminal = as_legacy(completed(
            &config,
            "legacy-terminal",
            json!({"theme": "legacy"}),
            json!({"theme": "legacy"}),
        ));
        let mut legacy_pending = pending(&config, "legacy-pending", json!({"locale": "fr"}));
        legacy_pending.version = LEGACY_CONFIG_TRANSACTION_VERSION;
        let legacy_journal = ConfigTransactionJournal {
            version: LEGACY_CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(&config),
            revision: 0,
            batches: vec![legacy_pending],
            updated_unix_ms: unix_ms(),
        };
        std::fs::write(
            active_path(&config),
            serde_json::to_vec_pretty(&legacy_journal).unwrap(),
        )
        .unwrap();
        let mut history_bytes = serde_json::to_vec(&legacy_terminal).unwrap();
        history_bytes.push(b'\n');
        std::fs::write(history_path(&config), history_bytes).unwrap();

        recover(&config).unwrap();

        assert_eq!(Preferences::load_or_default(&config).locale, "fr");
        assert!(!active_path(&config).exists());
        assert!(!active_recovery_path(&config).exists());
        let state = read_history(&config).unwrap();
        assert_eq!(
            state
                .recent
                .iter()
                .map(|row| (row.version, row.batch_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (CONFIG_TRANSACTION_VERSION, "legacy-terminal"),
                (CONFIG_TRANSACTION_VERSION, "legacy-pending"),
            ]
        );
        assert_eq!(state.dedupe.version, CONFIG_TRANSACTION_VERSION);
        assert_eq!(state.manifest.version, CONFIG_TRANSACTION_VERSION);
        assert_eq!(
            std::fs::read(dedupe_path(&config)).unwrap(),
            std::fs::read(dedupe_recovery_path(&config)).unwrap()
        );
        assert_eq!(
            std::fs::read(manifest_path(&config)).unwrap(),
            std::fs::read(manifest_recovery_path(&config)).unwrap()
        );
    }

    #[test]
    fn interrupted_v1_migration_adopts_immutable_orphan_and_keeps_old_retry() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        std::fs::create_dir_all(transaction_dir(&config)).unwrap();
        let rows = (0..4)
            .map(|index| {
                completed(
                    &config,
                    &format!("migration-{index}"),
                    json!({"theme": format!("theme-{index}")}),
                    json!({"theme": format!("theme-{index}")}),
                )
            })
            .collect::<Vec<_>>();

        let mut interrupted_manifest = ConfigArchiveManifest::empty(&config);
        publish_archive_segment(&config, &mut interrupted_manifest, &rows[..2]).unwrap();
        remove_durable(&manifest_path(&config)).unwrap();
        remove_durable(&manifest_recovery_path(&config)).unwrap();
        let legacy = ConfigTransactionDedupe {
            version: LEGACY_CONFIG_TRANSACTION_VERSION,
            config_dir: normalized(&config),
            revision: 7,
            batches: rows
                .iter()
                .cloned()
                .map(as_legacy)
                .map(|row| (row.batch_id.clone(), row))
                .collect(),
            order: rows.iter().map(|row| row.batch_id.clone()).collect(),
            updated_unix_ms: unix_ms(),
        };
        let legacy_bytes = serde_json::to_vec_pretty(&legacy).unwrap();
        std::fs::write(dedupe_path(&config), &legacy_bytes).unwrap();
        std::fs::write(dedupe_recovery_path(&config), &legacy_bytes).unwrap();
        let mut recent = Vec::new();
        for row in rows.iter().cloned().map(as_legacy) {
            serde_json::to_writer(&mut recent, &row).unwrap();
            recent.push(b'\n');
        }
        std::fs::write(history_path(&config), recent).unwrap();

        let state = read_history(&config).unwrap();
        assert_eq!(state.archived.len(), 2);
        assert_eq!(
            state
                .dedupe
                .order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["migration-2", "migration-3"]
        );
        assert_eq!(state.manifest.segments.len(), 1);
        assert_eq!(
            execute(
                &config,
                "electron-retry",
                "migration-0",
                900,
                "prefs.patch",
                json!({"theme": "theme-0"}),
            )
            .unwrap(),
            json!({"theme": "theme-0"})
        );
        assert!(!config.join("omegat.prefs.json").exists());
        assert_eq!(discover_archive_segments(&config).unwrap().len(), 1);
    }

    #[test]
    fn manifest_and_hot_index_publish_failures_stop_before_product_mutation() {
        for blocked in ["manifest.recovery.json", "dedupe.recovery.json"] {
            let temp = tempfile::tempdir().unwrap();
            let config = temp.path().join("config");
            std::fs::create_dir_all(transaction_dir(&config).join(blocked)).unwrap();
            let error = execute(
                &config,
                "electron",
                "blocked",
                901,
                "prefs.patch",
                json!({"locale": "fr"}),
            )
            .unwrap_err();
            assert!(
                error.contains("archive manifest") || error.contains("dedupe"),
                "{error}"
            );
            assert!(!config.join("omegat.prefs.json").exists());
        }
    }
}
