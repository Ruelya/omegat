// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable FIFO for filesystem fingerprints awaiting an external refresh.
//!
//! Refresh batches and team conflict transactions use the same versioned
//! [`omegat_team::TransactionEnvelope`]. A config-scoped pointer identifies the
//! one project that was active in Electron without making `omegat-core` depend
//! on `omegat-team`.

use omegat_team::{
    write_json_atomic, TransactionEnvelope, TransactionRendererAck, TransactionStatus,
    REQUEST_CANCELLED_CODE, TRANSACTION_ENVELOPE_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const QUEUE_VERSION: u8 = 2;
const JOURNAL_FILE: &str = "external-refresh.json";
const HISTORY_FILE: &str = "external-refresh-history.ndjson";
const ACTIVE_FILE: &str = "external-refresh-active.json";
static BATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshBatch {
    #[serde(default = "external_refresh_operation")]
    pub operation: String,
    pub paths: Vec<String>,
    pub fingerprints: BTreeMap<String, Option<String>>,
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_result: Option<Value>,
}

pub type RefreshEnvelope = TransactionEnvelope<RefreshBatch>;

fn external_refresh_operation() -> String {
    "project.external-refresh".into()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RefreshJournal {
    version: u8,
    project_root: PathBuf,
    app_instance: String,
    generation: u64,
    batches: Vec<RefreshEnvelope>,
    updated_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ActiveProject {
    version: u8,
    project_root: PathBuf,
    app_instance: String,
    updated_unix_ms: u128,
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

fn journal_path(root: &Path) -> PathBuf {
    root.join(".repositories")
        .join("transactions")
        .join(JOURNAL_FILE)
}

fn history_path(root: &Path) -> PathBuf {
    root.join(".repositories")
        .join("transactions")
        .join(HISTORY_FILE)
}

fn active_path(config_dir: &Path) -> PathBuf {
    config_dir.join("transactions").join(ACTIVE_FILE)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read refresh journal {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("parse refresh journal {}: {error}", path.display()))
}

fn sync_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync refresh journal {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    write_json_atomic(path, value)
        .map_err(|error| format!("publish refresh journal {}: {error}", path.display()))
}

fn remove_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove refresh journal {}: {error}",
            path.display()
        )),
    }
}

fn append_history(root: &Path, envelope: &RefreshEnvelope) -> Result<(), String> {
    let path = history_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| format!("refresh history has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create refresh history {}: {error}", parent.display()))?;
    let mut history = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open refresh history {}: {error}", path.display()))?;
    serde_json::to_writer(&mut history, envelope)
        .map_err(|error| format!("serialize refresh history: {error}"))?;
    history
        .write_all(b"\n")
        .and_then(|_| history.sync_all())
        .map_err(|error| format!("write refresh history {}: {error}", path.display()))?;
    sync_parent(&path)
}

fn compaction_checkpoint(point: &str) -> Result<(), String> {
    if std::env::var("OMEGAT_TEST_REFRESH_COMPACTION_POINT").as_deref() != Ok(point) {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_REFRESH_COMPACTION_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create compaction marker parent: {error}"))?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(format!("create compaction marker: {error}")),
    };
    writeln!(file, "{point}").map_err(|error| format!("write compaction marker: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("sync compaction marker: {error}"))?;
    sync_parent(&marker)?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn compact_acknowledged_batches(root: &Path, journal: &mut RefreshJournal) -> Result<bool, String> {
    let terminal = journal
        .batches
        .iter()
        .filter(|envelope| !envelope.status.is_recoverable())
        .cloned()
        .collect::<Vec<_>>();
    for envelope in &terminal {
        append_history(root, envelope)?;
    }
    if terminal.is_empty() {
        return Ok(false);
    }
    // The marker is durable before the process parks. A packaged E2E can
    // SIGKILL Electron's whole process group at this exact boundary and prove
    // that the still-authoritative source queue retains every recoverable row.
    compaction_checkpoint("after_archive_fsync")?;
    journal
        .batches
        .retain(|envelope| envelope.status.is_recoverable());
    Ok(true)
}

fn load_journal(root: &Path) -> Result<Option<RefreshJournal>, String> {
    let Some(journal) = read_json::<RefreshJournal>(&journal_path(root))? else {
        return Ok(None);
    };
    if journal.version != QUEUE_VERSION {
        return Err(format!(
            "unsupported refresh journal version {}",
            journal.version
        ));
    }
    if normalized(&journal.project_root) != normalized(root) {
        remove_file(&journal_path(root))?;
        return Ok(None);
    }
    for envelope in &journal.batches {
        envelope
            .validate_for_root(root)
            .map_err(|error| format!("refresh transaction envelope: {error}"))?;
        if envelope.payload.operation != external_refresh_operation() {
            return Err(format!(
                "refresh transaction {} has operation {}",
                envelope.batch_id, envelope.payload.operation
            ));
        }
        match envelope.status {
            TransactionStatus::Pending if envelope.payload.committed_result.is_some() => {
                return Err(format!(
                    "pending refresh {} carries a committed result",
                    envelope.batch_id
                ));
            }
            TransactionStatus::SidecarCommitted => {
                let result = envelope.payload.committed_result.as_ref().ok_or_else(|| {
                    format!(
                        "sidecar-committed refresh {} has no durable result",
                        envelope.batch_id
                    )
                })?;
                let items = committed_result_items(result);
                if !envelope.verify_product(result, items) {
                    return Err(format!(
                        "sidecar-committed refresh {} product receipt mismatch",
                        envelope.batch_id
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(Some(journal))
}

fn write_active(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    write_json(
        &active_path(config_dir),
        &ActiveProject {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(root),
            app_instance: app_instance.to_string(),
            updated_unix_ms: unix_ms(),
        },
    )
}

fn cancel_queue(root: &Path) -> Result<(), String> {
    let Some(mut journal) = load_journal(root)? else {
        return Ok(());
    };
    for envelope in &mut journal.batches {
        match envelope.status {
            TransactionStatus::Pending => {
                envelope.transition(TransactionStatus::Cancelled, None);
                append_history(root, envelope)?;
            }
            TransactionStatus::SidecarCommitted => {
                // The product result and receipt already crossed their atomic
                // boundary. A project-generation switch may drop the renderer
                // rebind, but it must not relabel committed product work as
                // cancelled or make it replayable.
                envelope.transition(TransactionStatus::Completed, None);
                append_history(root, envelope)?;
            }
            _ => append_history(root, envelope)?,
        }
    }
    remove_file(&journal_path(root))
}

fn select_active_project(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    if let Some(active) = read_json::<ActiveProject>(&active_path(config_dir))? {
        if active.version != TRANSACTION_ENVELOPE_VERSION {
            return Err(format!(
                "unsupported active refresh journal version {}",
                active.version
            ));
        }
        if normalized(&active.project_root) != normalized(root) {
            // Opening a different root is a project-generation boundary.  A
            // batch from the formerly active root must never be replayed when
            // that project happens to be opened again later.
            cancel_queue(&active.project_root)?;
        }
    }
    write_active(config_dir, root, app_instance)
}

pub fn pending(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
) -> Result<Vec<RefreshEnvelope>, String> {
    select_active_project(config_dir, root, app_instance)?;
    let Some(mut journal) = load_journal(root)? else {
        return Ok(Vec::new());
    };
    // Compact only terminal renderer-acknowledged records. Persist that
    // compaction before adopting a replacement process so an unacknowledged
    // sidecar receipt or a pending FIFO tail can never be dropped or have an
    // old terminal generation rewritten as current.
    if compact_acknowledged_batches(root, &mut journal)? {
        if std::env::var("OMEGAT_TEST_ABORT_REFRESH_COMPACTION_AFTER_ARCHIVE").as_deref() == Ok("1")
        {
            // Fault injection after durable history append but before the
            // compacted queue's atomic replacement. The original journal must
            // remain the recovery source, including its unacknowledged receipt
            // and pending tail.
            std::process::abort();
        }
        if journal.batches.is_empty() {
            remove_file(&journal_path(root))?;
            return Ok(Vec::new());
        }
        journal.updated_unix_ms = unix_ms();
        write_json(&journal_path(root), &journal)?;
        // write_json_atomic has already fsynced the replacement and its parent
        // directory. A process-group SIGKILL from this checkpoint must leave
        // this compacted queue authoritative.
        compaction_checkpoint("after_queue_rename")?;
        if std::env::var("OMEGAT_TEST_ABORT_REFRESH_COMPACTION_AFTER_QUEUE_RENAME").as_deref()
            == Ok("1")
        {
            // The compacted queue is already durable. A replacement process
            // must adopt its unacknowledged receipt and pending tail rather
            // than treating process death as an acknowledgement.
            std::process::abort();
        }
    }
    if journal.app_instance == app_instance && journal.generation != generation {
        // The same Electron process advanced its project generation.  This is
        // a reload/open boundary, not crash recovery.
        cancel_queue(root)?;
        return Ok(Vec::new());
    }
    if journal.app_instance != app_instance {
        // A new Electron process may adopt only the queue for the same active
        // project root.  Re-stamp its renderer generation before replay.
        journal.app_instance = app_instance.to_string();
        journal.generation = generation;
        for envelope in &mut journal.batches {
            // Keep updated_unix_ms as the durable cross-backend dispatch key.
            // Renderer adoption changes ownership, not FIFO creation order.
            envelope.generation = generation;
        }
        journal.updated_unix_ms = unix_ms();
        write_json(&journal_path(root), &journal)?;
    }
    Ok(journal.batches)
}

pub fn enqueue(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    paths: Vec<String>,
    fingerprints: BTreeMap<String, Option<String>>,
    sources: Vec<String>,
) -> Result<RefreshEnvelope, String> {
    let _ = pending(config_dir, root, app_instance, generation)?;
    let mut journal = load_journal(root)?.unwrap_or_else(|| RefreshJournal {
        version: QUEUE_VERSION,
        project_root: normalized(root),
        app_instance: app_instance.to_string(),
        generation,
        batches: Vec::new(),
        updated_unix_ms: unix_ms(),
    });
    if let Some(existing) = journal.batches.iter_mut().find(|batch| {
        batch.status == TransactionStatus::Pending && batch.payload.fingerprints == fingerprints
    }) {
        for source in sources {
            if !existing.payload.sources.contains(&source) {
                existing.payload.sources.push(source);
            }
        }
        existing.payload.sources.sort();
        existing.touch();
        let result = existing.clone();
        journal.updated_unix_ms = unix_ms();
        write_json(&journal_path(root), &journal)?;
        return Ok(result);
    }
    let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let batch = TransactionEnvelope::pending(
        root,
        generation,
        format!("refresh-{}-{}-{sequence}", unix_ms(), std::process::id()),
        RefreshBatch {
            operation: external_refresh_operation(),
            paths,
            fingerprints,
            sources,
            committed_result: None,
        },
    );
    journal.batches.push(batch.clone());
    journal.updated_unix_ms = unix_ms();
    write_json(&journal_path(root), &journal)?;
    Ok(batch)
}

pub fn complete(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
    status: TransactionStatus,
    error_code: Option<i32>,
) -> Result<Vec<RefreshEnvelope>, String> {
    let pending = pending(config_dir, root, app_instance, generation)?;
    let Some(first) = pending.first() else {
        return Ok(Vec::new());
    };
    if first.batch_id != batch_id {
        return Err(format!(
            "refresh FIFO head is {}, not {batch_id}",
            first.batch_id
        ));
    }
    let mut journal = load_journal(root)?
        .ok_or_else(|| "refresh journal disappeared before completion".to_string())?;
    journal.batches[0].transition(status, error_code);
    write_json(&journal_path(root), &journal)?;
    append_history(root, &journal.batches[0])?;
    journal.batches.remove(0);
    if journal.batches.is_empty() {
        remove_file(&journal_path(root))?;
        return Ok(Vec::new());
    }
    journal.updated_unix_ms = unix_ms();
    let remaining = journal.batches.clone();
    write_json(&journal_path(root), &journal)?;
    Ok(remaining)
}

pub fn checkpoint_sidecar_commit(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
    committed_result: &Value,
) -> Result<RefreshEnvelope, String> {
    let pending = pending(config_dir, root, app_instance, generation)?;
    let Some(first) = pending.first() else {
        return Err(format!("refresh batch {batch_id} is no longer pending"));
    };
    if first.batch_id != batch_id {
        return Err(format!(
            "refresh FIFO head is {}, not {batch_id}",
            first.batch_id
        ));
    }
    if first.status == TransactionStatus::SidecarCommitted {
        return Ok(first.clone());
    }
    let mut journal = load_journal(root)?
        .ok_or_else(|| "refresh journal disappeared before checkpoint".to_string())?;
    journal.batches[0].payload.committed_result = Some(committed_result.clone());
    journal.batches[0].commit_product(
        TransactionStatus::SidecarCommitted,
        committed_result,
        committed_result_items(committed_result),
    )?;
    let checkpoint = journal.batches[0].clone();
    journal.updated_unix_ms = unix_ms();
    write_json(&journal_path(root), &journal)?;
    Ok(checkpoint)
}

fn committed_result_items(result: &Value) -> u64 {
    result
        .get("entry_list")
        .and_then(Value::as_array)
        .map_or(0, |entries| entries.len() as u64)
}

pub fn request_cancelled(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
) -> Result<Vec<RefreshEnvelope>, String> {
    complete(
        config_dir,
        root,
        app_instance,
        generation,
        batch_id,
        TransactionStatus::RequestCancelled,
        Some(REQUEST_CANCELLED_CODE),
    )
}

fn acknowledged_in_history(root: &Path, generation: u64, batch_id: &str) -> Result<bool, String> {
    let Ok(history) = std::fs::read_to_string(history_path(root)) else {
        return Ok(false);
    };
    for line in history.lines().rev().filter(|line| !line.trim().is_empty()) {
        let envelope: RefreshEnvelope = serde_json::from_str(line)
            .map_err(|error| format!("parse refresh history: {error}"))?;
        if envelope.batch_id == batch_id {
            return Ok(envelope.generation == generation
                && !envelope.status.is_recoverable()
                && envelope.payload.operation == external_refresh_operation());
        }
    }
    Ok(false)
}

pub fn acknowledge(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
    outcome: &str,
) -> Result<TransactionRendererAck, String> {
    let pending = pending(config_dir, root, app_instance, generation)?;
    if let Some(first) = pending.first() {
        if first.batch_id != batch_id {
            return Err(format!(
                "refresh renderer receipt is {}, not {batch_id}",
                first.batch_id
            ));
        }
        let (status, error_code) = if outcome == "cancelled" {
            (
                TransactionStatus::RequestCancelled,
                Some(REQUEST_CANCELLED_CODE),
            )
        } else {
            (TransactionStatus::Completed, None)
        };
        complete(
            config_dir,
            root,
            app_instance,
            generation,
            batch_id,
            status,
            error_code,
        )?;
        return Ok(TransactionRendererAck {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(root),
            generation,
            batch_id: batch_id.to_string(),
            acknowledged: true,
            already_acknowledged: false,
        });
    }
    if acknowledged_in_history(root, generation, batch_id)? {
        return Ok(TransactionRendererAck {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(root),
            generation,
            batch_id: batch_id.to_string(),
            acknowledged: true,
            already_acknowledged: true,
        });
    }
    Err(format!("unknown refresh renderer receipt {batch_id}"))
}

pub fn discard(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    cancel_queue(root)?;
    if let Some(active) = read_json::<ActiveProject>(&active_path(config_dir))? {
        if normalized(&active.project_root) == normalized(root)
            && active.app_instance == app_instance
        {
            remove_file(&active_path(config_dir))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprints(value: &str) -> BTreeMap<String, Option<String>> {
        BTreeMap::from([("/project/source/a.txt".to_string(), Some(value.to_string()))])
    }

    #[test]
    fn adopts_only_same_root_after_process_restart_and_keeps_fifo() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        let one = enqueue(
            &config,
            &first,
            "electron-one",
            9,
            vec!["/project/source/a.txt".into()],
            fingerprints("one"),
            vec!["native".into()],
        )
        .unwrap();
        let two = enqueue(
            &config,
            &first,
            "electron-one",
            9,
            vec!["/project/source/a.txt".into()],
            fingerprints("two"),
            vec!["sidecar".into()],
        )
        .unwrap();
        let adopted = pending(&config, &first, "electron-two", 1).unwrap();
        assert_eq!(
            adopted
                .iter()
                .map(|batch| batch.batch_id.as_str())
                .collect::<Vec<_>>(),
            vec![one.batch_id.as_str(), two.batch_id.as_str()]
        );
        assert!(adopted.iter().all(|batch| batch.generation == 1));
        assert_eq!(
            complete(
                &config,
                &first,
                "electron-two",
                1,
                &one.batch_id,
                TransactionStatus::RequestCancelled,
                Some(REQUEST_CANCELLED_CODE),
            )
            .unwrap()[0]
                .batch_id,
            two.batch_id
        );

        assert!(pending(&config, &second, "electron-two", 2)
            .unwrap()
            .is_empty());
        assert!(pending(&config, &first, "electron-two", 3)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn same_process_generation_change_and_completion_never_revive_batches() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        let batch = enqueue(
            &config,
            &root,
            "electron",
            4,
            vec!["/project/source/a.txt".into()],
            fingerprints("one"),
            vec!["native".into()],
        )
        .unwrap();
        checkpoint_sidecar_commit(
            &config,
            &root,
            "electron",
            4,
            &batch.batch_id,
            &serde_json::json!({"entry_list": [{"source": "committed"}]}),
        )
        .unwrap();
        assert_eq!(
            pending(&config, &root, "electron", 4).unwrap()[0].status,
            TransactionStatus::SidecarCommitted
        );
        assert!(complete(
            &config,
            &root,
            "electron",
            4,
            &batch.batch_id,
            TransactionStatus::Completed,
            None,
        )
        .unwrap()
        .is_empty());
        assert!(pending(&config, &root, "electron", 4).unwrap().is_empty());
        let terminal: RefreshEnvelope = serde_json::from_str(
            std::fs::read_to_string(history_path(&root))
                .unwrap()
                .lines()
                .last()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(terminal.batch_id, batch.batch_id);
        assert_eq!(terminal.status, TransactionStatus::Completed);

        enqueue(
            &config,
            &root,
            "electron",
            4,
            vec!["/project/source/a.txt".into()],
            fingerprints("two"),
            vec!["sidecar".into()],
        )
        .unwrap();
        assert!(pending(&config, &root, "electron", 5).unwrap().is_empty());
    }
}
