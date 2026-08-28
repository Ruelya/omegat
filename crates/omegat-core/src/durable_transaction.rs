// SPDX-License-Identifier: GPL-3.0-or-later

//! Generic crash-safe transaction workflow.
//!
//! A workflow combines [`crate::durable_fifo`] active state with
//! [`crate::segmented_history`] terminal history.  Domain crates own product
//! mutation and rollback, while this module owns the ordering invariants around
//! enqueue, dispatch, acknowledgement, cancellation intent, terminal
//! publication, compaction, and restartable legacy-history import.

use crate::durable_fifo::{
    self, DurableFifoEntry, DurableFifoLayout, DurableFifoLock, DurableFifoState,
    DurableOwnerClaim, LegacyFifoState, LegacyOwnerClaim, OwnerClaimError, OwnerClaimOutcome,
};
use crate::segmented_history::{
    SegmentedHistory, SegmentedHistoryLayout, SegmentedHistoryOptions, SegmentedHistoryRecord,
    SegmentedHistoryStatus,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TRANSACTION_ENVELOPE_VERSION: u8 = 1;
pub const REQUEST_CANCELLED_CODE: i32 = -32800;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    CancellationPending,
    SidecarCommitted,
    Completed,
    Cancelled,
    RequestCancelled,
}

impl TransactionStatus {
    pub fn is_recoverable(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::CancellationPending | Self::SidecarCommitted
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionCommit {
    pub manifest_sha256: String,
    pub manifest_items: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionEnvelope<T> {
    pub version: u8,
    pub project_root: PathBuf,
    pub generation: u64,
    pub batch_id: String,
    pub status: TransactionStatus,
    pub error_code: Option<i32>,
    pub updated_unix_ms: u128,
    pub payload: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<TransactionCommit>,
}

impl<T> TransactionEnvelope<T> {
    pub fn pending(
        project_root: &Path,
        generation: u64,
        batch_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: durable_fifo::normalized(project_root),
            generation,
            batch_id: batch_id.into(),
            status: TransactionStatus::Pending,
            error_code: None,
            updated_unix_ms: unix_ms(),
            payload,
            commit: None,
        }
    }

    pub fn validate_for_root(&self, root: &Path) -> Result<(), String> {
        if self.version != TRANSACTION_ENVELOPE_VERSION {
            return Err(format!(
                "unsupported transaction envelope version {}",
                self.version
            ));
        }
        if self.batch_id.is_empty() {
            return Err("transaction envelope batch id is empty".into());
        }
        if durable_fifo::normalized(&self.project_root) != durable_fifo::normalized(root) {
            return Err(format!(
                "transaction envelope root {} does not match {}",
                self.project_root.display(),
                root.display()
            ));
        }
        if matches!(
            self.status,
            TransactionStatus::CancellationPending | TransactionStatus::RequestCancelled
        ) && self.error_code != Some(REQUEST_CANCELLED_CODE)
        {
            return Err("cancelling transaction envelope must carry -32800".into());
        }
        if self.status == TransactionStatus::SidecarCommitted && self.commit.is_none() {
            return Err("sidecar-committed transaction envelope has no product receipt".into());
        }
        if matches!(
            self.status,
            TransactionStatus::Pending
                | TransactionStatus::CancellationPending
                | TransactionStatus::Cancelled
                | TransactionStatus::RequestCancelled
        ) && self.commit.is_some()
        {
            return Err(format!(
                "{:?} transaction envelope must not carry a product receipt",
                self.status
            ));
        }
        Ok(())
    }

    pub fn restamp_generation(&mut self, generation: u64) {
        self.generation = generation;
        self.touch();
    }

    pub fn transition(&mut self, status: TransactionStatus, error_code: Option<i32>) {
        self.status = status;
        self.error_code = error_code;
        if matches!(
            status,
            TransactionStatus::Pending
                | TransactionStatus::CancellationPending
                | TransactionStatus::Cancelled
                | TransactionStatus::RequestCancelled
        ) {
            self.commit = None;
        }
        self.touch();
    }

    pub fn commit_product<M: Serialize>(
        &mut self,
        status: TransactionStatus,
        manifest: &M,
        manifest_items: u64,
    ) -> Result<(), String> {
        if !matches!(
            status,
            TransactionStatus::SidecarCommitted | TransactionStatus::Completed
        ) {
            return Err(format!("cannot attach a product receipt to {status:?}"));
        }
        let bytes = serde_json::to_vec(manifest)
            .map_err(|error| format!("serialize transaction product manifest: {error}"))?;
        self.commit = Some(TransactionCommit {
            manifest_sha256: format!("{:x}", Sha256::digest(bytes)),
            manifest_items,
        });
        self.status = status;
        self.error_code = None;
        self.touch();
        Ok(())
    }

    pub fn verify_product<M: Serialize>(&self, manifest: &M, manifest_items: u64) -> bool {
        let Ok(bytes) = serde_json::to_vec(manifest) else {
            return false;
        };
        self.commit.as_ref()
            == Some(&TransactionCommit {
                manifest_sha256: format!("{:x}", Sha256::digest(bytes)),
                manifest_items,
            })
    }

    pub fn touch(&mut self) {
        self.updated_unix_ms = unix_ms();
    }
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize transaction state: {error}"))?;
    crate::durable_file::replace(path, &bytes)
        .map_err(|error| format!("publish transaction state {}: {error}", path.display()))
}

pub fn normalized(path: &Path) -> PathBuf {
    durable_fifo::normalized(path)
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Storage-neutral state used by the workflow to enforce transition ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableTransactionPhase {
    Pending,
    CancellationPending,
    Committed,
    Terminal,
    Acknowledged,
}

impl DurableTransactionPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal | Self::Acknowledged)
    }

    pub fn is_dispatchable(self) -> bool {
        matches!(self, Self::Committed)
    }

    /// Whether replacing an active row with `next` preserves the unified
    /// transaction state machine.
    pub fn allows(self, next: Self) -> bool {
        match self {
            Self::Pending => matches!(
                next,
                Self::Pending
                    | Self::CancellationPending
                    | Self::Committed
                    | Self::Terminal
                    | Self::Acknowledged
            ),
            Self::CancellationPending => {
                matches!(
                    next,
                    Self::CancellationPending | Self::Terminal | Self::Acknowledged
                )
            }
            Self::Committed => matches!(
                next,
                Self::Committed | Self::CancellationPending | Self::Terminal | Self::Acknowledged
            ),
            Self::Terminal => matches!(next, Self::Terminal | Self::Acknowledged),
            Self::Acknowledged => next == Self::Acknowledged,
        }
    }
}

/// Domain record contract for the shared transaction state machine.
pub trait DurableTransactionRecord:
    DurableFifoEntry + SegmentedHistoryRecord + Clone + PartialEq
{
    fn transaction_id(&self) -> &str {
        self.durable_fifo_id()
    }

    fn transaction_phase(&self) -> DurableTransactionPhase;

    fn validate_history_for_scope(&self, _scope: &Path) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DurableTransactionLayout {
    pub fifo: DurableFifoLayout,
    pub history: SegmentedHistoryLayout,
    pub migration_seed_file: String,
}

impl DurableTransactionLayout {
    pub fn named(name: &str) -> Self {
        Self {
            fifo: DurableFifoLayout::default(),
            history: SegmentedHistoryLayout::named(&format!("{name}-history")),
            migration_seed_file: format!(".{name}-history-migration.ndjson"),
        }
    }

    fn validate(&self) -> Result<(), String> {
        self.history.validate()?;
        let name = &self.migration_seed_file;
        if name.is_empty()
            || Path::new(name).file_name().and_then(|value| value.to_str()) != Some(name.as_str())
        {
            return Err(format!(
                "unsafe durable transaction migration seed name {name}"
            ));
        }
        Ok(())
    }
}

impl Default for DurableTransactionLayout {
    fn default() -> Self {
        Self {
            fifo: DurableFifoLayout::default(),
            history: SegmentedHistoryLayout::default(),
            migration_seed_file: ".history-legacy-migration.ndjson".into(),
        }
    }
}

/// Result of an idempotent renderer acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableAcknowledgement<T> {
    Published(T),
    AlreadyPublished(T),
}

/// Generic record stored in a config-scoped cross-root discovery directory.
pub trait DurableScopeDiscoveryRecord: DeserializeOwned {
    fn discovery_scope(&self) -> &Path;
    fn discovery_updated_unix_ms(&self) -> u128;
    fn validate_discovery_record(&self) -> Result<(), String>;
}

/// Deduplicate scopes and order their oldest owner timestamp first.
///
/// Config-scoped receipt discovery calls this after collecting all live and
/// detached owner pointers. Acknowledging one root refreshes its pointer, so a
/// rediscovery gives every other root a turn before that root's FIFO tail.
pub fn fair_scope_order(candidates: impl IntoIterator<Item = (PathBuf, u128)>) -> Vec<PathBuf> {
    let mut scopes = BTreeMap::<PathBuf, u128>::new();
    for (scope, updated_unix_ms) in candidates {
        scopes
            .entry(durable_fifo::normalized(&scope))
            .and_modify(|updated| *updated = (*updated).max(updated_unix_ms))
            .or_insert(updated_unix_ms);
    }
    let mut scopes = scopes.into_iter().collect::<Vec<_>>();
    scopes.sort_by(|(left_scope, left_updated), (right_scope, right_updated)| {
        left_updated
            .cmp(right_updated)
            .then_with(|| left_scope.cmp(right_scope))
    });
    scopes.into_iter().map(|(scope, _)| scope).collect()
}

/// Read, validate, deduplicate, and fairly order a directory of scope records.
///
/// Hidden replacement candidates and non-JSON entries are never considered.
/// A malformed durable record fails closed instead of silently hiding a root.
pub fn discover_scopes<T: DurableScopeDiscoveryRecord>(
    directory: &Path,
) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read durable transaction discovery directory {}: {error}",
                directory.display()
            ))
        }
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read durable transaction discovery entry {}: {error}",
                directory.display()
            )
        })?;
        if !entry
            .file_type()
            .map_err(|error| {
                format!(
                    "inspect durable transaction discovery entry {}: {error}",
                    entry.path().display()
                )
            })?
            .is_file()
        {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with('.')
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|error| {
            format!(
                "read durable transaction discovery record {}: {error}",
                path.display()
            )
        })?;
        let record: T = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "parse durable transaction discovery record {}: {error}",
                path.display()
            )
        })?;
        record.validate_discovery_record().map_err(|error| {
            format!(
                "invalid durable transaction discovery record {}: {error}",
                path.display()
            )
        })?;
        if record.discovery_scope().as_os_str().is_empty() {
            return Err(format!(
                "invalid durable transaction discovery record {}: scope is empty",
                path.display()
            ));
        }
        candidates.push((
            record.discovery_scope().to_path_buf(),
            record.discovery_updated_unix_ms(),
        ));
    }
    Ok(fair_scope_order(candidates))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableOwnerIdentity {
    pub app_instance: String,
    pub process_id: u32,
    pub generation: u64,
}

impl DurableOwnerIdentity {
    pub fn new(
        app_instance: impl Into<String>,
        process_id: u32,
        generation: u64,
    ) -> Result<Self, String> {
        let identity = Self {
            app_instance: app_instance.into(),
            process_id,
            generation,
        };
        if identity.app_instance.is_empty() || identity.process_id == 0 || identity.generation == 0
        {
            return Err(
                "durable owner identity requires app instance, process id, and generation".into(),
            );
        }
        Ok(identity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableOwnerRetry {
    pub timeout: Duration,
    pub max_owner_deaths: usize,
    pub poll_interval: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableOwnerElection {
    pub claim: DurableOwnerClaim,
    pub previous_owner_process_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableOwnerElectionError {
    Busy,
    Live(DurableOwnerClaim),
    RetryRejected {
        previous_owner_process_id: u32,
        current_owner: DurableOwnerClaim,
    },
    Cancelled,
    TimedOut(Option<DurableOwnerClaim>),
    Durable(String),
}

impl std::fmt::Display for DurableOwnerElectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("durable transaction coordinator is locked"),
            Self::Live(claim) => write!(
                formatter,
                "transaction dispatcher is owned by live app {} (pid {})",
                claim.app_instance, claim.process_id
            ),
            Self::RetryRejected {
                previous_owner_process_id,
                current_owner,
            } => write!(
                formatter,
                "replacement retry after owner pid {previous_owner_process_id} exited was rejected: \
                 transaction dispatcher is owned by live app {} (pid {})",
                current_owner.app_instance, current_owner.process_id
            ),
            Self::Cancelled => formatter.write_str("durable owner election was cancelled"),
            Self::TimedOut(Some(claim)) => write!(
                formatter,
                "timed out waiting for transaction dispatcher {} (pid {})",
                claim.app_instance, claim.process_id
            ),
            Self::TimedOut(None) => {
                formatter.write_str("timed out waiting for durable transaction coordinator")
            }
            Self::Durable(error) => formatter.write_str(error),
        }
    }
}

/// Elect one dispatcher, optionally waiting through multiple owner deaths.
///
/// Every claim attempt and domain recovery callback runs under the same OS
/// lock. Cancellation is observed only while waiting and therefore cannot
/// mutate, release, or partially replace the current durable owner.
pub fn elect_owner_with_legacy<F, A, C, P, W>(
    directory: &Path,
    scope: &Path,
    layout: &DurableFifoLayout,
    identity: &DurableOwnerIdentity,
    retry: Option<DurableOwnerRetry>,
    decode_legacy: F,
    process_is_alive: A,
    mut is_cancelled: C,
    mut prepare_locked: P,
    mut owner_waiting: W,
) -> Result<DurableOwnerElection, DurableOwnerElectionError>
where
    F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String> + Copy,
    A: Fn(u32) -> bool,
    C: FnMut() -> bool,
    P: FnMut() -> Result<(), String>,
    W: FnMut(&DurableOwnerClaim) -> Result<(), String>,
{
    let deadline = retry.map(|options| Instant::now() + options.timeout);
    let poll_interval = retry
        .map(|options| options.poll_interval)
        .filter(|interval| !interval.is_zero())
        .unwrap_or(Duration::from_millis(25));
    let mut previous_owner_process_ids = Vec::new();
    let mut last_live_owner = None;
    loop {
        if is_cancelled() {
            return Err(DurableOwnerElectionError::Cancelled);
        }
        let lock = loop {
            match DurableFifoLock::try_acquire(directory, &layout.lock_file)
                .map_err(DurableOwnerElectionError::Durable)?
            {
                Some(lock) => break lock,
                None if retry.is_none() => return Err(DurableOwnerElectionError::Busy),
                None => {
                    if is_cancelled() {
                        return Err(DurableOwnerElectionError::Cancelled);
                    }
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        return Err(DurableOwnerElectionError::TimedOut(last_live_owner.clone()));
                    }
                    std::thread::sleep(poll_interval);
                }
            }
        };
        prepare_locked().map_err(DurableOwnerElectionError::Durable)?;
        let outcome = durable_fifo::claim_owner_with_legacy(
            directory,
            scope,
            layout,
            &identity.app_instance,
            identity.process_id,
            identity.generation,
            decode_legacy,
            &process_is_alive,
        );
        drop(lock);
        match outcome {
            Ok(OwnerClaimOutcome::Retained(claim))
            | Ok(OwnerClaimOutcome::Published { claim, .. }) => {
                return Ok(DurableOwnerElection {
                    claim,
                    previous_owner_process_ids,
                })
            }
            Err(OwnerClaimError::Durable(error)) => {
                return Err(DurableOwnerElectionError::Durable(error))
            }
            Err(OwnerClaimError::Live(claim)) => {
                if retry.is_none() {
                    return Err(DurableOwnerElectionError::Live(claim));
                }
                let options = retry.expect("checked above");
                if previous_owner_process_ids.len() >= options.max_owner_deaths {
                    return Err(DurableOwnerElectionError::RetryRejected {
                        previous_owner_process_id: previous_owner_process_ids
                            .last()
                            .copied()
                            .unwrap_or(claim.process_id),
                        current_owner: claim,
                    });
                }
                owner_waiting(&claim).map_err(DurableOwnerElectionError::Durable)?;
                last_live_owner = Some(claim.clone());
                while process_is_alive(claim.process_id) {
                    if is_cancelled() {
                        return Err(DurableOwnerElectionError::Cancelled);
                    }
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        return Err(DurableOwnerElectionError::TimedOut(Some(claim)));
                    }
                    std::thread::sleep(poll_interval);
                }
                previous_owner_process_ids.push(claim.process_id);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableCoordinatorLockMode {
    Try,
    Wait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableCoordinatorOpenError {
    Locked,
    Durable(String),
}

impl std::fmt::Display for DurableCoordinatorOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => formatter.write_str("durable transaction coordinator is locked"),
            Self::Durable(error) => formatter.write_str(error),
        }
    }
}

/// One opened transaction queue and its exact segmented history.
///
/// Callers hold the domain OS lock for the lifetime of this value.
pub struct DurableTransactionWorkflow<T: DurableTransactionRecord> {
    directory: PathBuf,
    scope: PathBuf,
    layout: DurableTransactionLayout,
    queue: DurableFifoState<T>,
    history: SegmentedHistory<T>,
    imported_legacy_history: bool,
}

/// One locked generic transaction workflow.
///
/// Queue/history recovery and the OS exclusion guard share this lifetime, so a
/// domain cannot accidentally reopen state outside the lock or split owner
/// validation from FIFO-head selection.
pub struct DurableTransactionCoordinator<T: DurableTransactionRecord> {
    directory: PathBuf,
    scope: PathBuf,
    layout: DurableTransactionLayout,
    _lock: DurableFifoLock,
    waited: bool,
    workflow: DurableTransactionWorkflow<T>,
}

impl<T: DurableTransactionRecord> DurableTransactionCoordinator<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn open_with_legacy<Q, H, C, P, W>(
        directory: &Path,
        scope: &Path,
        layout: DurableTransactionLayout,
        options: SegmentedHistoryOptions,
        lock_mode: DurableCoordinatorLockMode,
        decode_queue: Q,
        legacy_history: H,
        checkpoint: &mut C,
        prepare_locked: &mut P,
        wait_hook: &mut W,
    ) -> Result<Self, DurableCoordinatorOpenError>
    where
        Q: Fn(&[u8]) -> Result<Option<LegacyFifoState<T>>, String>,
        H: FnOnce() -> Result<Vec<T>, String>,
        C: FnMut(Option<&T>, &str) -> Result<(), String>,
        P: FnMut() -> Result<(), String>,
        W: FnMut() -> Result<(), String>,
    {
        let (lock, waited) = match DurableFifoLock::try_acquire(directory, &layout.fifo.lock_file)
            .map_err(DurableCoordinatorOpenError::Durable)?
        {
            Some(lock) => (lock, false),
            None if lock_mode == DurableCoordinatorLockMode::Try => {
                return Err(DurableCoordinatorOpenError::Locked)
            }
            None => {
                wait_hook().map_err(DurableCoordinatorOpenError::Durable)?;
                (
                    DurableFifoLock::acquire(directory, &layout.fifo.lock_file)
                        .map_err(DurableCoordinatorOpenError::Durable)?,
                    true,
                )
            }
        };
        prepare_locked().map_err(DurableCoordinatorOpenError::Durable)?;
        let workflow = DurableTransactionWorkflow::open_with_legacy(
            directory,
            scope,
            layout.clone(),
            options,
            decode_queue,
            legacy_history,
            checkpoint,
        )
        .map_err(DurableCoordinatorOpenError::Durable)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            scope: durable_fifo::normalized(scope),
            layout,
            _lock: lock,
            waited,
            workflow,
        })
    }

    pub fn open(
        directory: &Path,
        scope: &Path,
        layout: DurableTransactionLayout,
        options: SegmentedHistoryOptions,
        lock_mode: DurableCoordinatorLockMode,
    ) -> Result<Self, DurableCoordinatorOpenError> {
        Self::open_with_legacy(
            directory,
            scope,
            layout,
            options,
            lock_mode,
            |_| Ok(None),
            || Ok(Vec::new()),
            &mut |_, _| Ok(()),
            &mut || Ok(()),
            &mut || Ok(()),
        )
    }

    pub fn waited(&self) -> bool {
        self.waited
    }

    pub fn workflow(&self) -> &DurableTransactionWorkflow<T> {
        &self.workflow
    }

    pub fn workflow_mut(&mut self) -> &mut DurableTransactionWorkflow<T> {
        &mut self.workflow
    }

    pub fn load_owner_with_legacy<F>(
        &self,
        decode_legacy: F,
    ) -> Result<Option<DurableOwnerClaim>, String>
    where
        F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String>,
    {
        durable_fifo::load_owner_with_legacy(
            &self.directory,
            &self.scope,
            &self.layout.fifo,
            decode_legacy,
        )
    }

    pub fn verify_owner_with_legacy<F>(
        &self,
        identity: &DurableOwnerIdentity,
        decode_legacy: F,
    ) -> Result<DurableOwnerClaim, String>
    where
        F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String>,
    {
        let owner = self
            .load_owner_with_legacy(decode_legacy)?
            .ok_or_else(|| "transaction dispatcher owner disappeared".to_string())?;
        if owner.app_instance != identity.app_instance
            || owner.process_id != identity.process_id
            || owner.generation != identity.generation
        {
            return Err(format!(
                "transaction dispatcher owner changed to {} (pid {}, generation {})",
                owner.app_instance, owner.process_id, owner.generation
            ));
        }
        Ok(owner)
    }

    /// Release the current owner only after the active FIFO is empty.
    pub fn release_idle_owner_with_legacy<F>(&mut self, decode_legacy: F) -> Result<bool, String>
    where
        F: Fn(&[u8]) -> Result<Option<LegacyOwnerClaim>, String> + Copy,
    {
        if !self.workflow.queue().batches.is_empty() {
            return Ok(false);
        }
        let Some(owner) = self.load_owner_with_legacy(decode_legacy)? else {
            return Ok(false);
        };
        durable_fifo::release_owner_with_legacy(
            &self.directory,
            &self.scope,
            &self.layout.fifo,
            &owner,
            decode_legacy,
        )
    }
}

impl<T: DurableTransactionRecord> DurableTransactionWorkflow<T> {
    /// Open both stores, restartably importing a former append-only history.
    ///
    /// `legacy_history` is evaluated only when neither the segmented store nor
    /// its durable migration seed exists.  The seed is removed only after every
    /// row is present in segmented history.
    pub fn open_with_legacy<Q, H, C>(
        directory: &Path,
        scope: &Path,
        layout: DurableTransactionLayout,
        options: SegmentedHistoryOptions,
        decode_queue: Q,
        legacy_history: H,
        checkpoint: &mut C,
    ) -> Result<Self, String>
    where
        Q: Fn(&[u8]) -> Result<Option<LegacyFifoState<T>>, String>,
        H: FnOnce() -> Result<Vec<T>, String>,
        C: FnMut(Option<&T>, &str) -> Result<(), String>,
    {
        layout.validate()?;
        std::fs::create_dir_all(directory).map_err(|error| {
            format!(
                "create durable transaction directory {}: {error}",
                directory.display()
            )
        })?;
        let queue = durable_fifo::load_with_legacy(directory, scope, &layout.fifo, decode_queue)?;
        let seed = directory.join(&layout.migration_seed_file);
        let unified_exists = SegmentedHistory::<T>::has_durable_state(directory, &layout.history);
        let mut imported_legacy_history = seed.is_file();
        if !unified_exists && !seed.is_file() {
            let records = legacy_history()?;
            if !records.is_empty() {
                write_seed(&seed, &records)?;
                imported_legacy_history = true;
            }
        }
        let owner = queue.batches.first().cloned();
        let mut history_checkpoint = |point: &str| checkpoint(owner.as_ref(), point);
        let mut history = SegmentedHistory::open_with(
            directory,
            scope,
            layout.history.clone(),
            options,
            &mut history_checkpoint,
        )?;
        if seed.is_file() {
            for record in read_seed::<T>(&seed, scope)? {
                history.import_legacy([record], &mut |point| checkpoint(owner.as_ref(), point))?;
            }
            remove_durable(&seed)?;
        }
        Ok(Self {
            directory: directory.to_path_buf(),
            scope: durable_fifo::normalized(scope),
            layout,
            queue,
            history,
            imported_legacy_history,
        })
    }

    pub fn open(
        directory: &Path,
        scope: &Path,
        layout: DurableTransactionLayout,
        options: SegmentedHistoryOptions,
    ) -> Result<Self, String> {
        Self::open_with_legacy(
            directory,
            scope,
            layout,
            options,
            |_| Ok(None),
            || Ok(Vec::new()),
            &mut |_, _| Ok(()),
        )
    }

    pub fn queue(&self) -> &DurableFifoState<T> {
        &self.queue
    }

    pub fn queue_mut(&mut self) -> &mut DurableFifoState<T> {
        &mut self.queue
    }

    pub fn into_queue(self) -> DurableFifoState<T> {
        self.queue
    }

    pub fn into_history(self) -> SegmentedHistory<T> {
        self.history
    }

    pub fn history_status(&self) -> SegmentedHistoryStatus {
        self.history.status()
    }

    pub fn recent_history(&self) -> Vec<T> {
        self.history.recent()
    }

    pub fn history_records(&self, transaction_id: &str) -> Result<Vec<T>, String> {
        self.history.records_for(transaction_id)
    }

    pub fn imported_legacy_history(&self) -> bool {
        self.imported_legacy_history
    }

    /// Insert or replace one record without changing its FIFO position.
    pub fn upsert(&mut self, record: T) -> Result<(), String> {
        let id = record.transaction_id();
        if id.is_empty() {
            return Err("durable transaction id is empty".into());
        }
        if let Some(existing) = self
            .queue
            .batches
            .iter_mut()
            .find(|existing| existing.transaction_id() == id)
        {
            let current = existing.transaction_phase();
            let next = record.transaction_phase();
            if !current.allows(next) {
                return Err(format!(
                    "invalid durable transaction transition {current:?} -> {next:?} for {id}"
                ));
            }
            *existing = record;
        } else {
            self.queue.batches.push(record);
        }
        Ok(())
    }

    pub fn remove(&mut self, transaction_id: &str) -> Option<T> {
        self.queue
            .batches
            .iter()
            .position(|record| record.transaction_id() == transaction_id)
            .map(|index| self.queue.batches.remove(index))
    }

    pub fn persist_queue(&mut self) -> Result<(), String> {
        durable_fifo::persist(
            &self.directory,
            &self.scope,
            &self.layout.fifo,
            &mut self.queue,
        )
    }

    pub fn persist_or_clear_queue(&mut self) -> Result<(), String> {
        if self.queue.batches.is_empty() {
            durable_fifo::clear(&self.directory, &self.layout.fifo)
        } else {
            self.persist_queue()
        }
    }

    pub fn clear_queue(&mut self) -> Result<(), String> {
        self.queue.batches.clear();
        durable_fifo::clear(&self.directory, &self.layout.fifo)
    }

    pub fn append_history<C>(&mut self, record: T, checkpoint: &mut C) -> Result<bool, String>
    where
        C: FnMut(&str) -> Result<(), String>,
    {
        self.history.append_with(record, checkpoint)
    }

    /// Return the sole terminal decision for an id.
    ///
    /// Repeated byte-equivalent rows are accepted for migration compatibility;
    /// contradictory terminal decisions fail closed.
    pub fn terminal_record(&self, transaction_id: &str) -> Result<Option<T>, String> {
        let mut terminal = None;
        for record in self.history.records_for(transaction_id)? {
            if !record.transaction_phase().is_terminal() {
                continue;
            }
            match &terminal {
                Some(existing) if existing == &record => {}
                Some(_) => {
                    return Err(format!(
                        "durable transaction terminal result disagrees for {transaction_id}"
                    ))
                }
                None => terminal = Some(record),
            }
        }
        Ok(terminal)
    }

    /// Append a terminal decision exactly once.
    pub fn append_terminal<C>(&mut self, terminal: T, checkpoint: &mut C) -> Result<bool, String>
    where
        C: FnMut(&str) -> Result<(), String>,
    {
        if !terminal.transaction_phase().is_terminal() {
            return Err(format!(
                "durable transaction {} is not terminal",
                terminal.transaction_id()
            ));
        }
        if let Some(existing) = self.terminal_record(terminal.transaction_id())? {
            if existing != terminal {
                return Err(format!(
                    "durable transaction terminal result disagrees for {}",
                    terminal.transaction_id()
                ));
            }
            return Ok(false);
        }
        self.append_history(terminal, checkpoint)
    }

    pub fn dispatch_head<P>(&self, mut is_dispatchable: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        self.queue
            .batches
            .iter()
            .find(|record| is_dispatchable(record))
            .cloned()
    }

    /// Discover one exact receipt without changing queue order or ownership.
    pub fn receipt<P>(&self, transaction_id: &str, mut is_receipt: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        self.queue
            .batches
            .iter()
            .find(|record| record.transaction_id() == transaction_id && is_receipt(record))
            .cloned()
    }

    /// Persist a cancellation intent in-place, preserving the original FIFO
    /// position even when the target is not yet the dispatch head.
    pub fn persist_cancellation_intent(
        &mut self,
        transaction_id: &str,
        cancelling: T,
    ) -> Result<(), String> {
        if cancelling.transaction_id() != transaction_id
            || cancelling.transaction_phase() != DurableTransactionPhase::CancellationPending
        {
            return Err(format!(
                "invalid durable cancellation intent for {transaction_id}"
            ));
        }
        if !self
            .queue
            .batches
            .iter()
            .any(|record| record.transaction_id() == transaction_id)
        {
            return Err(format!(
                "unknown durable transaction cancellation target {transaction_id}"
            ));
        }
        self.upsert(cancelling)?;
        self.persist_queue()
    }

    /// Atomically converge a dispatch acknowledgement.
    ///
    /// The terminal is first retained in the active queue, then archived, then
    /// its domain cleanup runs, and only then is the queue row removed. A crash
    /// at any boundary leaves enough state for `compact_terminals` to finish.
    pub fn acknowledge_head<P, C, K>(
        &mut self,
        transaction_id: &str,
        terminal: T,
        mut is_dispatchable: P,
        cleanup: &mut C,
        checkpoint: &mut K,
    ) -> Result<DurableAcknowledgement<T>, String>
    where
        P: FnMut(&T) -> bool,
        C: FnMut(&T) -> Result<(), String>,
        K: FnMut(&str) -> Result<(), String>,
    {
        if terminal.transaction_id() != transaction_id
            || !terminal.transaction_phase().is_terminal()
        {
            return Err(format!(
                "invalid durable transaction terminal acknowledgement {transaction_id}"
            ));
        }
        if let Some(existing) = self.terminal_record(transaction_id)? {
            if existing != terminal {
                return Err(format!(
                    "durable transaction acknowledgement disagrees for {transaction_id}"
                ));
            }
            if self
                .queue
                .batches
                .iter()
                .any(|record| record.transaction_id() == transaction_id)
            {
                cleanup(&existing)?;
                self.remove(transaction_id);
                self.persist_or_clear_queue()?;
                checkpoint("after_ack_queue_compaction")?;
            }
            return Ok(DurableAcknowledgement::AlreadyPublished(existing));
        }

        let parked_terminal = self
            .queue
            .batches
            .iter()
            .find(|record| {
                record.transaction_id() == transaction_id
                    && record.transaction_phase().is_terminal()
            })
            .cloned();
        if let Some(parked) = parked_terminal {
            if parked != terminal {
                return Err(format!(
                    "durable transaction acknowledgement disagrees for {transaction_id}"
                ));
            }
        } else {
            let Some(head) = self.dispatch_head(&mut is_dispatchable) else {
                return Err(format!(
                    "unknown durable transaction acknowledgement {transaction_id}"
                ));
            };
            if head.transaction_id() != transaction_id {
                return Err(format!(
                    "durable transaction FIFO head is {}, not {transaction_id}",
                    head.transaction_id()
                ));
            }
            self.upsert(terminal.clone())?;
            self.persist_queue()?;
            checkpoint("after_terminal_queue_publish")?;
        }
        self.append_terminal(terminal.clone(), checkpoint)?;
        checkpoint("after_terminal_history_publish")?;
        cleanup(&terminal)?;
        self.remove(transaction_id);
        self.persist_or_clear_queue()?;
        checkpoint("after_ack_queue_compaction")?;
        Ok(DurableAcknowledgement::Published(terminal))
    }

    /// Archive and remove every terminal active row.
    ///
    /// Queue publication deliberately happens even when the resulting queue is
    /// empty. This closes the archive/queue-rename crash window before replicas
    /// are unlinked.
    pub fn compact_terminals<P, C, K>(
        &mut self,
        mut is_terminal: P,
        cleanup: &mut C,
        checkpoint: &mut K,
    ) -> Result<usize, String>
    where
        P: FnMut(&T) -> bool,
        C: FnMut(&T) -> Result<(), String>,
        K: FnMut(&str) -> Result<(), String>,
    {
        let terminal = self
            .queue
            .batches
            .iter()
            .filter(|record| is_terminal(record))
            .cloned()
            .collect::<Vec<_>>();
        if terminal.is_empty() {
            if self.queue.batches.is_empty() {
                durable_fifo::clear(&self.directory, &self.layout.fifo)?;
            }
            return Ok(0);
        }
        for record in &terminal {
            self.append_terminal(record.clone(), checkpoint)?;
        }
        checkpoint("after_archive_fsync")?;
        self.queue
            .batches
            .retain(|record| !terminal.iter().any(|old| old == record));
        self.persist_queue()?;
        checkpoint("after_queue_rename")?;
        for record in &terminal {
            cleanup(record)?;
        }
        if self.queue.batches.is_empty() {
            durable_fifo::clear(&self.directory, &self.layout.fifo)?;
        }
        Ok(terminal.len())
    }
}

fn write_seed<T: Serialize>(path: &Path, records: &[T]) -> Result<(), String> {
    let mut bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut bytes, record)
            .map_err(|error| format!("serialize durable transaction migration seed: {error}"))?;
        bytes.push(b'\n');
    }
    crate::durable_file::replace(path, &bytes).map_err(|error| {
        format!(
            "publish durable transaction migration seed {}: {error}",
            path.display()
        )
    })
}

fn read_seed<T: DurableTransactionRecord>(path: &Path, scope: &Path) -> Result<Vec<T>, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "read durable transaction migration seed {}: {error}",
            path.display()
        )
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(format!(
            "durable transaction migration seed {} has a truncated final row",
            path.display()
        ));
    }
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
        .map(|line| {
            let mut record: T = serde_json::from_slice(line).map_err(|error| {
                format!(
                    "parse durable transaction migration seed {}: {error}",
                    path.display()
                )
            })?;
            SegmentedHistoryRecord::relocate(&mut record, scope, scope);
            record.validate_history_for_scope(scope)?;
            Ok(record)
        })
        .collect()
}

fn remove_durable(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            let parent = path.parent().ok_or_else(|| {
                format!(
                    "durable transaction migration path has no parent: {}",
                    path.display()
                )
            })?;
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!(
                        "sync durable transaction directory {}: {error}",
                        parent.display()
                    )
                })
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove durable transaction migration seed {}: {error}",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct Record {
        id: String,
        scope: PathBuf,
        phase: String,
        value: Value,
    }

    impl DurableFifoEntry for Record {
        fn durable_fifo_id(&self) -> &str {
            &self.id
        }

        fn validate_for_scope(&self, scope: &Path) -> Result<(), String> {
            if durable_fifo::normalized(&self.scope) == durable_fifo::normalized(scope) {
                Ok(())
            } else {
                Err("record scope mismatch".into())
            }
        }

        fn relocate(&mut self, _old_scope: &Path, new_scope: &Path) {
            self.scope = durable_fifo::normalized(new_scope);
        }
    }

    impl SegmentedHistoryRecord for Record {
        fn history_partition(&self) -> &str {
            &self.id
        }

        fn relocate(&mut self, _old_scope: &Path, new_scope: &Path) {
            self.scope = durable_fifo::normalized(new_scope);
        }
    }

    impl DurableTransactionRecord for Record {
        fn transaction_phase(&self) -> DurableTransactionPhase {
            match self.phase.as_str() {
                "pending" => DurableTransactionPhase::Pending,
                "cancelling" => DurableTransactionPhase::CancellationPending,
                "committed" => DurableTransactionPhase::Committed,
                "acknowledged" => DurableTransactionPhase::Acknowledged,
                _ => DurableTransactionPhase::Terminal,
            }
        }
    }

    fn record(scope: &Path, id: &str, phase: &str, value: u64) -> Record {
        Record {
            id: id.into(),
            scope: durable_fifo::normalized(scope),
            phase: phase.into(),
            value: value.into(),
        }
    }

    fn options() -> SegmentedHistoryOptions {
        SegmentedHistoryOptions {
            recent_limit: 2,
            hot_limit: 2,
            segment_record_limit: 1,
            generation_segment_limit: 3,
            generation_record_limit: 8,
            partition_prefix_hex: 2,
        }
    }

    #[test]
    fn acknowledgement_is_fifo_exactly_once_and_restartable() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = scope.join("transactions");
        let layout = DurableTransactionLayout::default();
        let mut workflow =
            DurableTransactionWorkflow::open(&directory, &scope, layout.clone(), options())
                .unwrap();
        workflow
            .upsert(record(&scope, "first", "committed", 1))
            .unwrap();
        workflow
            .upsert(record(&scope, "second", "committed", 2))
            .unwrap();
        workflow.persist_queue().unwrap();
        let error = workflow
            .acknowledge_head(
                "second",
                record(&scope, "second", "acknowledged", 2),
                |row| row.transaction_phase().is_dispatchable(),
                &mut |_| Ok(()),
                &mut |_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error, "durable transaction FIFO head is first, not second");
        let first = record(&scope, "first", "acknowledged", 1);
        assert!(matches!(
            workflow
                .acknowledge_head(
                    "first",
                    first.clone(),
                    |row| row.transaction_phase().is_dispatchable(),
                    &mut |_| Ok(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DurableAcknowledgement::Published(_)
        ));
        drop(workflow);

        let mut reopened: DurableTransactionWorkflow<Record> =
            DurableTransactionWorkflow::open(&directory, &scope, layout, options()).unwrap();
        assert_eq!(reopened.queue().batches[0].transaction_id(), "second");
        assert_eq!(
            reopened.terminal_record("first").unwrap(),
            Some(first.clone())
        );
        assert!(matches!(
            reopened
                .acknowledge_head(
                    "first",
                    first,
                    |row| row.transaction_phase().is_dispatchable(),
                    &mut |_| Ok(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DurableAcknowledgement::AlreadyPublished(_)
        ));
    }

    #[test]
    fn acknowledgement_recovers_both_terminal_publication_boundaries() {
        for stopped_at in [
            "after_terminal_queue_publish",
            "after_terminal_history_publish",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let scope = temp.path().join("scope");
            std::fs::create_dir_all(&scope).unwrap();
            let directory = scope.join("transactions");
            let layout = DurableTransactionLayout::default();
            let mut workflow =
                DurableTransactionWorkflow::open(&directory, &scope, layout.clone(), options())
                    .unwrap();
            workflow
                .upsert(record(&scope, "receipt", "committed", 1))
                .unwrap();
            workflow.persist_queue().unwrap();
            let terminal = record(&scope, "receipt", "acknowledged", 1);
            let error = workflow
                .acknowledge_head(
                    "receipt",
                    terminal.clone(),
                    |row| row.transaction_phase().is_dispatchable(),
                    &mut |_| Ok(()),
                    &mut |point| {
                        if point == stopped_at {
                            Err(format!("stopped at {point}"))
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
            assert_eq!(error, format!("stopped at {stopped_at}"));
            drop(workflow);

            let mut reopened: DurableTransactionWorkflow<Record> =
                DurableTransactionWorkflow::open(&directory, &scope, layout.clone(), options())
                    .unwrap();
            assert_eq!(
                reopened
                    .receipt("receipt", |row| row.transaction_phase().is_terminal())
                    .unwrap(),
                terminal
            );
            let mut cleanups = 0;
            let recovered = reopened
                .acknowledge_head(
                    "receipt",
                    terminal.clone(),
                    |row| row.transaction_phase().is_dispatchable(),
                    &mut |_| {
                        cleanups += 1;
                        Ok(())
                    },
                    &mut |_| Ok(()),
                )
                .unwrap();
            if stopped_at == "after_terminal_queue_publish" {
                assert!(matches!(recovered, DurableAcknowledgement::Published(_)));
            } else {
                assert!(matches!(
                    recovered,
                    DurableAcknowledgement::AlreadyPublished(_)
                ));
            }
            assert_eq!(cleanups, 1);
            assert!(reopened.queue().batches.is_empty());
            assert_eq!(
                reopened.terminal_record("receipt").unwrap(),
                Some(terminal.clone())
            );
            drop(reopened);

            let mut reopened: DurableTransactionWorkflow<Record> =
                DurableTransactionWorkflow::open(&directory, &scope, layout, options()).unwrap();
            assert!(matches!(
                reopened
                    .acknowledge_head(
                        "receipt",
                        terminal,
                        |row| row.transaction_phase().is_dispatchable(),
                        &mut |_| panic!("already-compacted receipt ran cleanup twice"),
                        &mut |_| Ok(()),
                    )
                    .unwrap(),
                DurableAcknowledgement::AlreadyPublished(_)
            ));
        }
    }

    #[test]
    fn cancellation_intent_keeps_position_and_compaction_converges() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = scope.join("transactions");
        let mut workflow = DurableTransactionWorkflow::open(
            &directory,
            &scope,
            DurableTransactionLayout::default(),
            options(),
        )
        .unwrap();
        workflow
            .upsert(record(&scope, "older", "committed", 1))
            .unwrap();
        workflow
            .upsert(record(&scope, "cancel", "committed", 2))
            .unwrap();
        workflow.persist_queue().unwrap();
        workflow
            .persist_cancellation_intent("cancel", record(&scope, "cancel", "cancelling", 2))
            .unwrap();
        assert_eq!(
            workflow
                .queue()
                .batches
                .iter()
                .map(|row| row.transaction_id())
                .collect::<Vec<_>>(),
            vec!["older", "cancel"]
        );
        workflow
            .upsert(record(&scope, "cancel", "cancelled", 2))
            .unwrap();
        workflow.persist_queue().unwrap();
        assert_eq!(
            workflow
                .compact_terminals(
                    |row| row.transaction_phase().is_terminal(),
                    &mut |_| Ok(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            1
        );
        assert_eq!(workflow.queue().batches.len(), 1);
        assert_eq!(
            workflow.terminal_record("cancel").unwrap().unwrap().phase,
            "cancelled"
        );
    }

    #[test]
    fn legacy_seed_survives_partial_import_and_scope_move() {
        let temp = tempfile::tempdir().unwrap();
        let old_scope = temp.path().join("old");
        std::fs::create_dir_all(&old_scope).unwrap();
        let directory = old_scope.join("transactions");
        std::fs::create_dir_all(&directory).unwrap();
        let layout = DurableTransactionLayout::default();
        let legacy = vec![
            record(&old_scope, "legacy-a", "acknowledged", 1),
            record(&old_scope, "legacy-b", "acknowledged", 2),
        ];
        write_seed(&directory.join(&layout.migration_seed_file), &legacy).unwrap();
        let new_scope = temp.path().join("new");
        std::fs::rename(&old_scope, &new_scope).unwrap();
        let directory = new_scope.join("transactions");
        let workflow: DurableTransactionWorkflow<Record> =
            DurableTransactionWorkflow::open_with_legacy(
                &directory,
                &new_scope,
                layout.clone(),
                options(),
                |_| Ok(None),
                || panic!("durable seed must suppress legacy rediscovery"),
                &mut |_, _| Ok(()),
            )
            .unwrap();
        assert!(workflow.imported_legacy_history());
        assert_eq!(
            workflow.terminal_record("legacy-a").unwrap().unwrap().scope,
            durable_fifo::normalized(&new_scope)
        );
        assert!(!directory.join(&layout.migration_seed_file).exists());
    }

    #[test]
    fn discovery_is_global_fifo_with_stable_scope_tiebreaking() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        let c = temp.path().join("c");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::create_dir_all(&c).unwrap();
        assert_eq!(
            fair_scope_order([
                (c.clone(), 30),
                (a.clone(), 20),
                (b.clone(), 20),
                (a.clone(), 10),
            ]),
            vec![
                durable_fifo::normalized(&a),
                durable_fifo::normalized(&b),
                durable_fifo::normalized(&c),
            ]
        );
    }

    const PROJECT_WRITERS: &[&str] = &[
        "entry.set",
        "project.save",
        "project.close",
        "project.reload",
        "project.compile",
        "project.import",
        "project.update",
        "team.mapping",
        "team.sync",
        "team.commit",
        "team.resolve",
        "align.write",
        "glossary.add",
        "search.replace",
        "spell.ignore",
        "spell.learn",
        "tmx.export",
        "wiki.import",
        "script.run",
        "script.slot",
        "align.run",
        "project.external-refresh",
    ];

    const CONFIG_WRITERS: &[&str] = &[
        "prefs.set",
        "prefs.patch",
        "aligner.configure",
        "spell.install",
    ];

    fn all_writers() -> impl Iterator<Item = &'static str> {
        PROJECT_WRITERS.iter().chain(CONFIG_WRITERS).copied()
    }

    #[test]
    fn every_writer_recovers_every_acknowledgement_crash_boundary() {
        for (sequence, operation) in all_writers().enumerate() {
            for stopped_at in [
                "after_terminal_queue_publish",
                "after_terminal_history_publish",
                "after_ack_queue_compaction",
            ] {
                let temp = tempfile::tempdir().unwrap();
                let scope = temp.path().join(format!("scope-{sequence}"));
                std::fs::create_dir_all(&scope).unwrap();
                let directory = scope.join("transactions");
                let layout = DurableTransactionLayout::default();
                let id = format!("{operation}-{stopped_at}");
                let mut workflow =
                    DurableTransactionWorkflow::open(&directory, &scope, layout.clone(), options())
                        .unwrap();
                let mut committed = record(&scope, &id, "committed", sequence as u64);
                committed.value = serde_json::json!({"operation": operation});
                workflow.upsert(committed).unwrap();
                workflow.persist_queue().unwrap();
                let mut terminal = record(&scope, &id, "acknowledged", sequence as u64);
                terminal.value = serde_json::json!({"operation": operation});
                let error = workflow
                    .acknowledge_head(
                        &id,
                        terminal.clone(),
                        |row| row.transaction_phase().is_dispatchable(),
                        &mut |_| Ok(()),
                        &mut |point| {
                            if point == stopped_at {
                                Err(format!("crash:{point}"))
                            } else {
                                Ok(())
                            }
                        },
                    )
                    .unwrap_err();
                assert_eq!(error, format!("crash:{stopped_at}"));
                drop(workflow);

                let mut recovered =
                    DurableTransactionWorkflow::open(&directory, &scope, layout, options())
                        .unwrap();
                recovered
                    .acknowledge_head(
                        &id,
                        terminal.clone(),
                        |row| row.transaction_phase().is_dispatchable(),
                        &mut |_| Ok(()),
                        &mut |_| Ok(()),
                    )
                    .unwrap();
                assert_eq!(recovered.history_records(&id).unwrap(), vec![terminal]);
                assert_eq!(
                    recovered
                        .queue()
                        .batches
                        .iter()
                        .filter(|row| row.transaction_id() == id)
                        .count(),
                    0
                );
            }
        }
    }

    #[test]
    fn every_writer_resumes_every_history_migration_crash_boundary() {
        const POINTS: &[&str] = &[
            "after_recent_append",
            "after_hot_append",
            "after_segment_candidate_write",
            "after_segment_candidate_fsync",
            "after_segment_rename",
            "after_segment_parent_fsync",
            "after_manifest_publish",
            "after_hot_prune",
            "after_generation_manifest_publish",
            "after_gc_delete",
        ];
        for (writer_index, operation) in all_writers().enumerate() {
            for point in POINTS {
                let temp = tempfile::tempdir().unwrap();
                let scope = temp.path().join(format!("migration-{writer_index}"));
                std::fs::create_dir_all(&scope).unwrap();
                let directory = scope.join("transactions");
                let layout = DurableTransactionLayout::default();
                let legacy = (0..8)
                    .map(|row| {
                        let mut record = record(
                            &scope,
                            &format!("{operation}-legacy-{row}"),
                            "acknowledged",
                            row,
                        );
                        record.value = serde_json::json!({
                            "operation": operation,
                            "sequence": row,
                        });
                        record
                    })
                    .collect::<Vec<_>>();
                let stopped = std::cell::Cell::new(false);
                let first = DurableTransactionWorkflow::open_with_legacy(
                    &directory,
                    &scope,
                    layout.clone(),
                    options(),
                    |_| Ok(None),
                    || Ok(legacy.clone()),
                    &mut |_, reached| {
                        if reached == *point && !stopped.replace(true) {
                            Err(format!("crash:{reached}"))
                        } else {
                            Ok(())
                        }
                    },
                );
                assert!(
                    stopped.get(),
                    "{operation} did not reach migration crash point {point}"
                );
                assert_eq!(first.err().unwrap(), format!("crash:{point}"));

                let recovered: DurableTransactionWorkflow<Record> =
                    DurableTransactionWorkflow::open_with_legacy(
                    &directory,
                    &scope,
                    layout,
                    options(),
                    |_| Ok(None),
                    || panic!("durable migration seed did not suppress legacy rediscovery"),
                    &mut |_, _| Ok(()),
                )
                    .unwrap();
                for expected in legacy {
                    assert_eq!(
                        recovered
                            .history_records(expected.transaction_id())
                            .unwrap(),
                        vec![expected]
                    );
                }
            }
        }
    }

    #[test]
    fn every_writer_fails_closed_on_equal_revision_replica_divergence() {
        for (sequence, operation) in all_writers().enumerate() {
            let temp = tempfile::tempdir().unwrap();
            let scope = temp.path().join(format!("replica-{sequence}"));
            std::fs::create_dir_all(&scope).unwrap();
            let directory = scope.join("transactions");
            let layout = DurableTransactionLayout::default();
            let id = format!("{operation}-replica");
            let mut workflow =
                DurableTransactionWorkflow::open(&directory, &scope, layout.clone(), options())
                    .unwrap();
            let mut pending = record(&scope, &id, "pending", sequence as u64);
            pending.value = serde_json::json!({"operation": operation, "copy": "recovery"});
            workflow.upsert(pending).unwrap();
            workflow.persist_queue().unwrap();
            drop(workflow);

            let mut primary: DurableFifoState<Record> = serde_json::from_slice(
                &std::fs::read(directory.join(&layout.fifo.primary_file)).unwrap(),
            )
            .unwrap();
            primary.batches[0].value["copy"] = Value::String("primary".into());
            std::fs::write(
                directory.join(&layout.fifo.primary_file),
                serde_json::to_vec_pretty(&primary).unwrap(),
            )
            .unwrap();
            let error =
                DurableTransactionWorkflow::<Record>::open(&directory, &scope, layout, options())
                    .err()
                    .unwrap();
            assert!(error.contains("replicas disagree at revision 1"));
        }
    }

    #[test]
    fn coordinator_handles_multi_generation_takeover_release_and_cancelled_wait() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("scope");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = scope.join("transactions");
        let layout = DurableTransactionLayout::default();
        let first = durable_fifo::claim_owner(
            &directory,
            &scope,
            &layout.fifo,
            "generation-one",
            101,
            1,
            |_| false,
        )
        .unwrap();
        let OwnerClaimOutcome::Published {
            claim: first_claim, ..
        } = first
        else {
            panic!("first owner was not published");
        };
        let liveness_checks = std::sync::atomic::AtomicU64::new(0);
        let election = elect_owner_with_legacy(
            &directory,
            &scope,
            &layout.fifo,
            &DurableOwnerIdentity::new("generation-two", 202, 2).unwrap(),
            Some(DurableOwnerRetry {
                timeout: Duration::from_secs(1),
                max_owner_deaths: 3,
                poll_interval: Duration::from_millis(1),
            }),
            |_| Ok(None),
            |pid| {
                pid == 101 && liveness_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2
            },
            || false,
            || Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(election.previous_owner_process_ids, vec![101]);
        assert_eq!(election.claim.generation, 2);
        assert!(election.claim.revision > first_claim.revision);

        let mut coordinator = DurableTransactionCoordinator::<Record>::open(
            &directory,
            &scope,
            layout.clone(),
            options(),
            DurableCoordinatorLockMode::Wait,
        )
        .unwrap();
        assert!(coordinator
            .release_idle_owner_with_legacy(|_| Ok(None))
            .unwrap());
        assert_eq!(
            durable_fifo::load_owner(&directory, &scope, &layout.fifo).unwrap(),
            None
        );
        drop(coordinator);

        let cancelled = elect_owner_with_legacy(
            &directory,
            &scope,
            &layout.fifo,
            &DurableOwnerIdentity::new("generation-three", 303, 3).unwrap(),
            Some(DurableOwnerRetry {
                timeout: Duration::from_secs(1),
                max_owner_deaths: 3,
                poll_interval: Duration::from_millis(1),
            }),
            |_| Ok(None),
            |_| false,
            || true,
            || panic!("cancelled election acquired and prepared the lock"),
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(cancelled, DurableOwnerElectionError::Cancelled);
        assert_eq!(
            durable_fifo::load_owner(&directory, &scope, &layout.fifo).unwrap(),
            None
        );
    }

    #[derive(Debug, Deserialize)]
    struct DiscoveryRecord {
        root: PathBuf,
        updated: u128,
        valid: bool,
    }

    impl DurableScopeDiscoveryRecord for DiscoveryRecord {
        fn discovery_scope(&self) -> &Path {
            &self.root
        }

        fn discovery_updated_unix_ms(&self) -> u128 {
            self.updated
        }

        fn validate_discovery_record(&self) -> Result<(), String> {
            if self.valid {
                Ok(())
            } else {
                Err("record was marked invalid".into())
            }
        }
    }

    #[test]
    fn generic_cross_root_discovery_is_fair_and_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("discovery");
        std::fs::create_dir_all(&directory).unwrap();
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        for (name, root, updated) in [
            ("a-old.json", &a, 1_u128),
            ("a-new.json", &a, 3),
            ("b.json", &b, 2),
        ] {
            std::fs::write(
                directory.join(name),
                serde_json::to_vec(&serde_json::json!({
                    "root": root,
                    "updated": updated,
                    "valid": true,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        std::fs::write(directory.join(".candidate.json"), b"{").unwrap();
        std::fs::write(directory.join("ignored.txt"), b"{").unwrap();
        assert_eq!(
            discover_scopes::<DiscoveryRecord>(&directory).unwrap(),
            vec![durable_fifo::normalized(&b), durable_fifo::normalized(&a)]
        );

        std::fs::write(
            directory.join("invalid.json"),
            serde_json::to_vec(&serde_json::json!({
                "root": a,
                "updated": 4,
                "valid": false,
            }))
            .unwrap(),
        )
        .unwrap();
        let error = discover_scopes::<DiscoveryRecord>(&directory).unwrap_err();
        assert!(error.contains("record was marked invalid"));
    }
}
