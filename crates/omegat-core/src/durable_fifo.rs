// SPDX-License-Identifier: GPL-3.0-or-later

//! Process-shared durable FIFO state.
//!
//! A queue has two equal-value replicas and a monotonically increasing
//! revision. Readers select the newest valid revision, repair a missing,
//! corrupt, migrated, or stale peer, and reject equal-revision disagreement.
//! The OS lock and renderer-owner election use the same directory and durable
//! replacement primitive, but remain independent from the queued payload.

use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DURABLE_FIFO_VERSION: u8 = 1;
pub const DURABLE_OWNER_VERSION: u8 = 1;
static OWNER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Domain payload contract required by the shared FIFO.
pub trait DurableFifoEntry:
    Clone + std::fmt::Debug + DeserializeOwned + PartialEq + Serialize
{
    fn durable_fifo_id(&self) -> &str;
    fn validate_for_scope(&self, scope: &Path) -> Result<(), String>;
    fn relocate(&mut self, old_scope: &Path, new_scope: &Path);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableFifoLayout {
    pub primary_file: String,
    pub recovery_file: String,
    pub lock_file: String,
    pub owner_file: String,
    pub owner_recovery_file: String,
}

impl Default for DurableFifoLayout {
    fn default() -> Self {
        Self {
            primary_file: "active.json".into(),
            recovery_file: ".active.recovery.json".into(),
            lock_file: "operation.lock".into(),
            owner_file: "renderer-owner.json".into(),
            owner_recovery_file: ".renderer-owner.recovery.json".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableFifoState<T> {
    pub version: u8,
    pub scope: PathBuf,
    pub revision: u64,
    pub batches: Vec<T>,
    pub updated_unix_ms: u128,
}

impl<T> DurableFifoState<T> {
    pub fn empty(scope: &Path) -> Self {
        Self {
            version: DURABLE_FIFO_VERSION,
            scope: normalized(scope),
            revision: 0,
            batches: Vec::new(),
            updated_unix_ms: unix_ms(),
        }
    }
}

/// Decoded state from a domain's former active format.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyFifoState<T> {
    pub scope: PathBuf,
    pub revision: u64,
    pub batches: Vec<T>,
    pub updated_unix_ms: u128,
}

struct Replica<T> {
    exists: bool,
    state: Option<DurableFifoState<T>>,
    migrated: bool,
    error: Option<String>,
}

/// Held process-wide filesystem exclusion for one FIFO directory.
pub struct DurableFifoLock {
    _file: File,
}

impl DurableFifoLock {
    pub fn acquire(directory: &Path, lock_file: &str) -> Result<Self, String> {
        let file = open_lock(directory, lock_file)?;
        file.lock_exclusive()
            .map_err(|error| format!("lock durable FIFO {}: {error}", directory.display()))?;
        Ok(Self { _file: file })
    }

    pub fn try_acquire(directory: &Path, lock_file: &str) -> Result<Option<Self>, String> {
        let file = open_lock(directory, lock_file)?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(format!(
                "lock durable FIFO {}: {error}",
                directory.display()
            )),
        }
    }
}

fn open_lock(directory: &Path, lock_file: &str) -> Result<File, String> {
    ensure_directory(directory)?;
    let path = directory.join(lock_file);
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("open durable FIFO lock {}: {error}", path.display()))
}

fn ensure_directory(directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| {
        format!(
            "create durable FIFO directory {}: {error}",
            directory.display()
        )
    })?;
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            format!(
                "sync durable FIFO directory {}: {error}",
                directory.display()
            )
        })
}

fn read_replica<T, F>(path: &Path, scope: &Path, decode_legacy: &F) -> Result<Replica<T>, String>
where
    T: DurableFifoEntry,
    F: Fn(&[u8]) -> Result<Option<LegacyFifoState<T>>, String>,
{
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Replica {
                exists: false,
                state: None,
                migrated: false,
                error: None,
            });
        }
        Err(error) => {
            return Err(format!(
                "read durable FIFO replica {}: {error}",
                path.display()
            ));
        }
    };

    let mut migrated = false;
    let mut state = match serde_json::from_slice::<DurableFifoState<T>>(&bytes) {
        Ok(state) if state.version == DURABLE_FIFO_VERSION => state,
        Ok(state) => {
            return Ok(Replica {
                exists: true,
                state: None,
                migrated: false,
                error: Some(format!(
                    "unsupported durable FIFO version {}",
                    state.version
                )),
            });
        }
        Err(current_error) => match decode_legacy(&bytes) {
            Ok(Some(legacy)) => {
                migrated = true;
                DurableFifoState {
                    version: DURABLE_FIFO_VERSION,
                    scope: legacy.scope,
                    revision: legacy.revision,
                    batches: legacy.batches,
                    updated_unix_ms: legacy.updated_unix_ms,
                }
            }
            Ok(None) => {
                return Ok(Replica {
                    exists: true,
                    state: None,
                    migrated: false,
                    error: Some(format!("parse current format: {current_error}")),
                });
            }
            Err(legacy_error) => {
                return Ok(Replica {
                    exists: true,
                    state: None,
                    migrated: false,
                    error: Some(format!(
                        "parse current format: {current_error}; legacy format: {legacy_error}"
                    )),
                });
            }
        },
    };

    if state.scope.as_os_str().is_empty() {
        return Ok(invalid_replica("durable FIFO scope is empty"));
    }
    let old_scope = state.scope.clone();
    if normalized(&old_scope) != normalized(scope) {
        for batch in &mut state.batches {
            batch.relocate(&old_scope, scope);
        }
        state.scope = normalized(scope);
        migrated = true;
    }
    if let Err(error) = validate_state(&state, scope) {
        return Ok(invalid_replica(error));
    }
    Ok(Replica {
        exists: true,
        state: Some(state),
        migrated,
        error: None,
    })
}

fn invalid_replica<T>(error: impl Into<String>) -> Replica<T> {
    Replica {
        exists: true,
        state: None,
        migrated: false,
        error: Some(error.into()),
    }
}

fn validate_state<T: DurableFifoEntry>(
    state: &DurableFifoState<T>,
    scope: &Path,
) -> Result<(), String> {
    if state.version != DURABLE_FIFO_VERSION {
        return Err(format!(
            "unsupported durable FIFO version {}",
            state.version
        ));
    }
    if normalized(&state.scope) != normalized(scope) {
        return Err(format!(
            "durable FIFO scope {} does not match {}",
            state.scope.display(),
            scope.display()
        ));
    }
    let mut ids = BTreeSet::new();
    for batch in &state.batches {
        let id = batch.durable_fifo_id();
        if id.is_empty() {
            return Err("durable FIFO batch id is empty".into());
        }
        if !ids.insert(id.to_owned()) {
            return Err(format!("durable FIFO contains duplicate batch {id}"));
        }
        batch.validate_for_scope(scope)?;
    }
    Ok(())
}

/// Load, migrate, select, and repair both active replicas.
pub fn load_with_legacy<T, F>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    decode_legacy: F,
) -> Result<DurableFifoState<T>, String>
where
    T: DurableFifoEntry,
    F: Fn(&[u8]) -> Result<Option<LegacyFifoState<T>>, String>,
{
    ensure_directory(directory)?;
    let replicas = [
        read_replica(&directory.join(&layout.primary_file), scope, &decode_legacy)?,
        read_replica(
            &directory.join(&layout.recovery_file),
            scope,
            &decode_legacy,
        )?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|replica| replica.state.as_ref())
        .cloned()
        .collect::<Vec<_>>();
    if valid.is_empty() {
        if replicas.iter().any(|replica| replica.exists) {
            let details = replicas
                .iter()
                .filter_map(|replica| replica.error.as_deref())
                .collect::<Vec<_>>()
                .join(" | ");
            return Err(format!(
                "both durable FIFO replicas are invalid in {}{}",
                directory.display(),
                if details.is_empty() {
                    String::new()
                } else {
                    format!(": {details}")
                }
            ));
        }
        return Ok(DurableFifoState::empty(scope));
    }
    valid.sort_by_key(|state| state.revision);
    let selected = valid.last().expect("non-empty valid FIFO replicas").clone();
    if valid
        .iter()
        .any(|state| state.revision == selected.revision && state != &selected)
    {
        return Err(format!(
            "durable FIFO replicas disagree at revision {} in {}",
            selected.revision,
            directory.display()
        ));
    }
    if replicas.iter().any(|replica| {
        replica.migrated
            || replica
                .state
                .as_ref()
                .map(|state| state != &selected)
                .unwrap_or(true)
    }) {
        publish_exact(directory, layout, &selected)?;
    }
    Ok(selected)
}

pub fn load<T>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
) -> Result<DurableFifoState<T>, String>
where
    T: DurableFifoEntry,
{
    load_with_legacy(directory, scope, layout, |_| Ok(None))
}

/// Increment and publish one queue revision to both replicas.
pub fn persist<T>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    state: &mut DurableFifoState<T>,
) -> Result<(), String>
where
    T: DurableFifoEntry,
{
    state.version = DURABLE_FIFO_VERSION;
    state.scope = normalized(scope);
    state.revision = state.revision.saturating_add(1);
    state.updated_unix_ms = unix_ms();
    validate_state(state, scope)?;
    publish_exact(directory, layout, state)
}

fn publish_exact<T: Serialize>(
    directory: &Path,
    layout: &DurableFifoLayout,
    state: &DurableFifoState<T>,
) -> Result<(), String> {
    ensure_directory(directory)?;
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("serialize durable FIFO: {error}"))?;
    for file in [&layout.recovery_file, &layout.primary_file] {
        let path = directory.join(file);
        crate::durable_file::replace(&path, &bytes)
            .map_err(|error| format!("publish durable FIFO replica {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Remove both replicas after the caller made the terminal result durable.
pub fn clear(directory: &Path, layout: &DurableFifoLayout) -> Result<(), String> {
    for file in [&layout.recovery_file, &layout.primary_file] {
        remove_durable(&directory.join(file))?;
    }
    Ok(())
}

fn remove_durable(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            let parent = path
                .parent()
                .ok_or_else(|| format!("durable FIFO path has no parent: {}", path.display()))?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!("sync durable FIFO directory {}: {error}", parent.display())
                })
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove durable FIFO replica {}: {error}",
            path.display()
        )),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableOwnerClaim {
    pub version: u8,
    pub scope: PathBuf,
    pub revision: u64,
    pub app_instance: String,
    pub process_id: u32,
    pub generation: u64,
    pub claim_id: String,
    pub updated_unix_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyOwnerClaim {
    pub scope: PathBuf,
    pub revision: u64,
    pub app_instance: String,
    pub process_id: u32,
    pub generation: u64,
    pub claim_id: String,
    pub updated_unix_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerClaimOutcome {
    Retained(DurableOwnerClaim),
    Published {
        previous_process_id: Option<u32>,
        claim: DurableOwnerClaim,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerClaimError {
    Durable(String),
    Live(DurableOwnerClaim),
}

impl std::fmt::Display for OwnerClaimError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Durable(error) => formatter.write_str(error),
            Self::Live(claim) => write!(
                formatter,
                "transaction dispatcher is owned by live app {} (pid {})",
                claim.app_instance, claim.process_id
            ),
        }
    }
}

fn owner_replica<F>(
    path: &Path,
    scope: &Path,
    decode_legacy: &F,
) -> Result<Replica<DurableOwnerClaim>, String>
where
    F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String>,
{
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Replica {
                exists: false,
                state: None,
                migrated: false,
                error: None,
            });
        }
        Err(error) => {
            return Err(format!(
                "read durable owner replica {}: {error}",
                path.display()
            ));
        }
    };
    let mut migrated = false;
    let mut claim = match serde_json::from_slice::<DurableOwnerClaim>(&bytes) {
        Ok(claim) if claim.version == DURABLE_OWNER_VERSION => claim,
        Ok(claim) => {
            return Ok(invalid_replica(format!(
                "unsupported durable owner version {}",
                claim.version
            )));
        }
        Err(current_error) => match decode_legacy(&bytes) {
            Ok(Some(legacy)) => {
                migrated = true;
                DurableOwnerClaim {
                    version: DURABLE_OWNER_VERSION,
                    scope: legacy.scope,
                    revision: legacy.revision,
                    app_instance: legacy.app_instance,
                    process_id: legacy.process_id,
                    generation: legacy.generation,
                    claim_id: legacy.claim_id,
                    updated_unix_ms: legacy.updated_unix_ms,
                }
            }
            Ok(None) => {
                return Ok(invalid_replica(format!(
                    "parse current owner format: {current_error}"
                )));
            }
            Err(legacy_error) => {
                return Ok(invalid_replica(format!(
                    "parse current owner format: {current_error}; legacy format: {legacy_error}"
                )));
            }
        },
    };
    if claim.scope.as_os_str().is_empty()
        || claim.app_instance.is_empty()
        || claim.process_id == 0
        || claim.generation == 0
        || claim.claim_id.is_empty()
    {
        return Ok(invalid_replica("invalid durable owner claim"));
    }
    if normalized(&claim.scope) != normalized(scope) {
        claim.scope = normalized(scope);
        migrated = true;
    }
    Ok(Replica {
        exists: true,
        state: Some(DurableFifoState {
            version: DURABLE_FIFO_VERSION,
            scope: claim.scope.clone(),
            revision: claim.revision,
            batches: vec![claim],
            updated_unix_ms: 0,
        }),
        migrated,
        error: None,
    })
}

pub fn load_owner_with_legacy<F>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    decode_legacy: F,
) -> Result<Option<DurableOwnerClaim>, String>
where
    F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String>,
{
    ensure_directory(directory)?;
    let replicas = [
        owner_replica(&directory.join(&layout.owner_file), scope, &decode_legacy)?,
        owner_replica(
            &directory.join(&layout.owner_recovery_file),
            scope,
            &decode_legacy,
        )?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|replica| replica.state.as_ref())
        .filter_map(|state| state.batches.first())
        .cloned()
        .collect::<Vec<_>>();
    if valid.is_empty() {
        if replicas.iter().any(|replica| replica.exists) {
            let details = replicas
                .iter()
                .filter_map(|replica| replica.error.as_deref())
                .collect::<Vec<_>>()
                .join(" | ");
            return Err(format!(
                "both durable owner replicas are invalid in {}{}",
                directory.display(),
                if details.is_empty() {
                    String::new()
                } else {
                    format!(": {details}")
                }
            ));
        }
        return Ok(None);
    }
    valid.sort_by_key(|claim| claim.revision);
    let selected = valid.last().expect("non-empty owner replicas").clone();
    if valid
        .iter()
        .any(|claim| claim.revision == selected.revision && claim != &selected)
    {
        return Err(format!(
            "durable owner replicas disagree at revision {} in {}",
            selected.revision,
            directory.display()
        ));
    }
    if replicas.iter().any(|replica| {
        replica.migrated
            || replica
                .state
                .as_ref()
                .and_then(|state| state.batches.first())
                .map(|claim| claim != &selected)
                .unwrap_or(true)
    }) {
        publish_owner_exact(directory, layout, &selected)?;
    }
    Ok(Some(selected))
}

pub fn load_owner(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
) -> Result<Option<DurableOwnerClaim>, String> {
    load_owner_with_legacy(directory, scope, layout, |_| Ok(None))
}

fn publish_owner_exact(
    directory: &Path,
    layout: &DurableFifoLayout,
    claim: &DurableOwnerClaim,
) -> Result<(), String> {
    ensure_directory(directory)?;
    let bytes = serde_json::to_vec_pretty(claim)
        .map_err(|error| format!("serialize durable owner claim: {error}"))?;
    for file in [&layout.owner_recovery_file, &layout.owner_file] {
        let path = directory.join(file);
        crate::durable_file::replace(&path, &bytes).map_err(|error| {
            format!(
                "publish durable owner claim replica {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

/// Claim an owner after the caller has acquired the FIFO lock.
pub fn claim_owner_with_legacy<F, A>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    app_instance: &str,
    process_id: u32,
    generation: u64,
    decode_legacy: F,
    process_is_alive: A,
) -> Result<OwnerClaimOutcome, OwnerClaimError>
where
    F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String>,
    A: Fn(u32) -> bool,
{
    if app_instance.is_empty() || process_id == 0 || generation == 0 {
        return Err(OwnerClaimError::Durable(
            "durable owner claim requires app instance, process id, and generation".into(),
        ));
    }
    let previous = load_owner_with_legacy(directory, scope, layout, decode_legacy)
        .map_err(OwnerClaimError::Durable)?;
    if let Some(previous) = &previous {
        if previous.app_instance != app_instance && process_is_alive(previous.process_id) {
            return Err(OwnerClaimError::Live(previous.clone()));
        }
        if previous.app_instance == app_instance
            && previous.process_id == process_id
            && previous.generation == generation
        {
            return Ok(OwnerClaimOutcome::Retained(previous.clone()));
        }
    }
    let revision = previous
        .as_ref()
        .map_or(1, |claim| claim.revision.saturating_add(1));
    let sequence = OWNER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let claim = DurableOwnerClaim {
        version: DURABLE_OWNER_VERSION,
        scope: normalized(scope),
        revision,
        app_instance: app_instance.to_owned(),
        process_id,
        generation,
        claim_id: format!("{}-{process_id}-{sequence}", unix_ms()),
        updated_unix_ms: unix_ms(),
    };
    publish_owner_exact(directory, layout, &claim).map_err(OwnerClaimError::Durable)?;
    Ok(OwnerClaimOutcome::Published {
        previous_process_id: previous.map(|claim| claim.process_id),
        claim,
    })
}

pub fn claim_owner<A>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    app_instance: &str,
    process_id: u32,
    generation: u64,
    process_is_alive: A,
) -> Result<OwnerClaimOutcome, OwnerClaimError>
where
    A: Fn(u32) -> bool,
{
    claim_owner_with_legacy(
        directory,
        scope,
        layout,
        app_instance,
        process_id,
        generation,
        |_| Ok(None),
        process_is_alive,
    )
}

pub fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct Entry {
        id: String,
        scope: PathBuf,
        value: Value,
    }

    impl DurableFifoEntry for Entry {
        fn durable_fifo_id(&self) -> &str {
            &self.id
        }

        fn validate_for_scope(&self, scope: &Path) -> Result<(), String> {
            if normalized(&self.scope) == normalized(scope) {
                Ok(())
            } else {
                Err("entry scope mismatch".into())
            }
        }

        fn relocate(&mut self, _old_scope: &Path, new_scope: &Path) {
            self.scope = normalized(new_scope);
        }
    }

    fn entry(scope: &Path, id: &str, value: u64) -> Entry {
        Entry {
            id: id.into(),
            scope: normalized(scope),
            value: value.into(),
        }
    }

    #[test]
    fn replicas_repair_by_revision_and_equal_revision_conflict_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = temp.path().join("fifo");
        let layout = DurableFifoLayout::default();
        let mut state = DurableFifoState::empty(&scope);
        state.batches.push(entry(&scope, "one", 1));
        persist(&directory, &scope, &layout, &mut state).unwrap();
        let expected = std::fs::read(directory.join(&layout.recovery_file)).unwrap();
        std::fs::write(directory.join(&layout.primary_file), b"{").unwrap();
        let repaired: DurableFifoState<Entry> = load(&directory, &scope, &layout).unwrap();
        assert_eq!(repaired.revision, 1);
        assert_eq!(
            std::fs::read(directory.join(&layout.primary_file)).unwrap(),
            expected
        );

        let mut conflict = repaired.clone();
        conflict.batches[0].value = Value::from(2);
        std::fs::write(
            directory.join(&layout.primary_file),
            serde_json::to_vec_pretty(&conflict).unwrap(),
        )
        .unwrap();
        let error = load::<Entry>(&directory, &scope, &layout).unwrap_err();
        assert!(error.contains("replicas disagree at revision 1"));
    }

    #[test]
    fn legacy_migration_is_restartable_after_only_recovery_was_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let old_scope = temp.path().join("old");
        std::fs::create_dir_all(&old_scope).unwrap();
        let new_scope = temp.path().join("new");
        let directory = old_scope.join("fifo");
        std::fs::create_dir_all(&directory).unwrap();
        let layout = DurableFifoLayout::default();
        let legacy = serde_json::json!({
            "old_scope": old_scope,
            "revision": 7,
            "items": [{"id": "legacy", "scope": old_scope, "value": 7}]
        });
        let bytes = serde_json::to_vec_pretty(&legacy).unwrap();
        std::fs::write(directory.join(&layout.primary_file), &bytes).unwrap();
        std::fs::write(directory.join(&layout.recovery_file), &bytes).unwrap();
        std::fs::rename(temp.path().join("old"), &new_scope).unwrap();
        let directory = new_scope.join("fifo");
        let decode = |bytes: &[u8]| {
            let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
            if value.get("items").is_none() {
                return Ok(None);
            }
            Ok(Some(LegacyFifoState {
                scope: serde_json::from_value(value["old_scope"].clone())
                    .map_err(|error| error.to_string())?,
                revision: value["revision"].as_u64().unwrap(),
                batches: serde_json::from_value(value["items"].clone())
                    .map_err(|error| error.to_string())?,
                updated_unix_ms: 1,
            }))
        };
        let migrated = load_with_legacy(&directory, &new_scope, &layout, decode).unwrap();
        assert_eq!(migrated.revision, 7);
        assert_eq!(migrated.batches[0].scope, normalized(&new_scope));

        // Recreate the interrupted state: the recovery replica is new while
        // primary still contains the equivalent old value at revision 7.
        std::fs::write(directory.join(&layout.primary_file), bytes).unwrap();
        let resumed = load_with_legacy(&directory, &new_scope, &layout, decode).unwrap();
        assert_eq!(resumed, migrated);
        assert_eq!(
            std::fs::read(directory.join(&layout.primary_file)).unwrap(),
            std::fs::read(directory.join(&layout.recovery_file)).unwrap()
        );
    }

    #[test]
    fn os_lock_and_owner_replicas_serialize_consecutive_takeovers() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = temp.path().join("fifo");
        let layout = DurableFifoLayout::default();
        let held = DurableFifoLock::acquire(&directory, &layout.lock_file).unwrap();
        assert!(DurableFifoLock::try_acquire(&directory, &layout.lock_file)
            .unwrap()
            .is_none());
        drop(held);

        let first = claim_owner(&directory, &scope, &layout, "first", 101, 1, |_| false).unwrap();
        assert!(matches!(first, OwnerClaimOutcome::Published { .. }));
        let live = claim_owner(&directory, &scope, &layout, "second", 202, 2, |pid| {
            pid == 101
        })
        .unwrap_err();
        assert!(matches!(live, OwnerClaimError::Live(_)));
        let second = claim_owner(&directory, &scope, &layout, "second", 202, 2, |_| false).unwrap();
        let OwnerClaimOutcome::Published { claim, .. } = second else {
            panic!("dead first owner was not replaced");
        };
        assert_eq!(claim.revision, 2);
        let third = claim_owner(&directory, &scope, &layout, "third", 303, 3, |_| false).unwrap();
        let OwnerClaimOutcome::Published { claim, .. } = third else {
            panic!("dead second owner was not replaced");
        };
        assert_eq!(claim.revision, 3);
        assert_eq!(
            std::fs::read(directory.join(&layout.owner_file)).unwrap(),
            std::fs::read(directory.join(&layout.owner_recovery_file)).unwrap()
        );
    }

    #[test]
    fn legacy_owner_migrates_restartably_and_equal_revision_conflict_is_closed() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = temp.path().join("fifo");
        std::fs::create_dir_all(&directory).unwrap();
        let layout = DurableFifoLayout::default();
        let legacy = serde_json::json!({
            "legacy_scope": scope,
            "app": "legacy-owner",
            "pid": 707,
            "generation": 8,
            "claim": "legacy-claim",
            "updated": 9
        });
        std::fs::write(
            directory.join(&layout.owner_file),
            serde_json::to_vec_pretty(&legacy).unwrap(),
        )
        .unwrap();
        let decode = |bytes: &[u8]| {
            let value: Value = match serde_json::from_slice(bytes) {
                Ok(value) => value,
                Err(_) => return Ok(None),
            };
            if value.get("legacy_scope").is_none() {
                return Ok(None);
            }
            Ok(Some(LegacyOwnerClaim {
                scope: serde_json::from_value(value["legacy_scope"].clone())
                    .map_err(|error| error.to_string())?,
                revision: 0,
                app_instance: value["app"].as_str().unwrap().into(),
                process_id: value["pid"].as_u64().unwrap() as u32,
                generation: value["generation"].as_u64().unwrap(),
                claim_id: value["claim"].as_str().unwrap().into(),
                updated_unix_ms: value["updated"].as_u64().unwrap() as u128,
            }))
        };
        let migrated = load_owner_with_legacy(&directory, &scope, &layout, decode)
            .unwrap()
            .unwrap();
        assert_eq!(migrated.app_instance, "legacy-owner");
        assert_eq!(
            std::fs::read(directory.join(&layout.owner_file)).unwrap(),
            std::fs::read(directory.join(&layout.owner_recovery_file)).unwrap()
        );

        let mut disagreement = migrated.clone();
        disagreement.generation = 9;
        std::fs::write(
            directory.join(&layout.owner_file),
            serde_json::to_vec_pretty(&disagreement).unwrap(),
        )
        .unwrap();
        let error = load_owner(&directory, &scope, &layout).unwrap_err();
        assert!(error.contains("owner replicas disagree at revision 0"));
    }
}
