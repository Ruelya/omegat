// SPDX-License-Identifier: GPL-3.0-or-later

//! Config-scoped refresh ownership and migration into the shared transaction FIFO.
//!
//! Refresh work no longer has an independent project journal. The only
//! project queue is `.repositories/transactions/active.json`, owned by
//! `omegat-team` together with product and team receipts. This module retains
//! the config-scoped Electron pointer and imports version-2
//! `external-refresh.json` installations idempotently.

use omegat_core::durable_transaction::{
    discover_scopes, write_json_atomic, DurableScopeDiscoveryRecord, TransactionEnvelope,
    TRANSACTION_ENVELOPE_VERSION,
};
use omegat_team::{TransactionRendererAck, TransactionRendererPayload, TransactionRendererReceipt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LEGACY_QUEUE_VERSION: u8 = 2;
const LEGACY_JOURNAL_FILE: &str = "external-refresh.json";
const LEGACY_HISTORY_FILE: &str = "external-refresh-history.ndjson";
const ACTIVE_DIRECTORY: &str = "external-refresh-active";
const LEGACY_ACTIVE_FILE: &str = "external-refresh-active.json";
static BATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub type RefreshEnvelope = TransactionEnvelope<TransactionRendererPayload>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LegacyRefreshJournal {
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
    #[serde(default)]
    generation: u64,
    updated_unix_ms: u128,
}

impl DurableScopeDiscoveryRecord for ActiveProject {
    fn discovery_scope(&self) -> &Path {
        &self.project_root
    }

    fn discovery_updated_unix_ms(&self) -> u128 {
        self.updated_unix_ms
    }

    fn validate_discovery_record(&self) -> Result<(), String> {
        if self.version != TRANSACTION_ENVELOPE_VERSION {
            return Err(format!(
                "unsupported active refresh owner version {}",
                self.version
            ));
        }
        if self.project_root.as_os_str().is_empty() || self.app_instance.is_empty() {
            return Err("active refresh owner requires project root and app instance".into());
        }
        Ok(())
    }
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

fn transaction_dir(root: &Path) -> PathBuf {
    root.join(".repositories").join("transactions")
}

fn legacy_journal_path(root: &Path) -> PathBuf {
    transaction_dir(root).join(LEGACY_JOURNAL_FILE)
}

fn legacy_history_path(root: &Path) -> PathBuf {
    transaction_dir(root).join(LEGACY_HISTORY_FILE)
}

fn legacy_per_app_active_path(config_dir: &Path, app_instance: &str) -> PathBuf {
    let owner = format!("{:x}", Sha256::digest(app_instance.as_bytes()));
    config_dir
        .join("transactions")
        .join(ACTIVE_DIRECTORY)
        .join(format!("{owner}.json"))
}

fn active_path(config_dir: &Path, app_instance: &str, root: &Path) -> PathBuf {
    let identity = format!("{}\0{}", app_instance, normalized(root).display());
    let owner = format!("{:x}", Sha256::digest(identity.as_bytes()));
    config_dir
        .join("transactions")
        .join(ACTIVE_DIRECTORY)
        .join(format!("{owner}.json"))
}

fn legacy_active_path(config_dir: &Path) -> PathBuf {
    config_dir.join("transactions").join(LEGACY_ACTIVE_FILE)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read refresh state {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("parse refresh state {}: {error}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    write_json_atomic(path, value)
        .map_err(|error| format!("publish refresh owner {}: {error}", path.display()))
}

fn sync_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync refresh state {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn remove_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove refresh state {}: {error}", path.display())),
    }
}

fn legacy_migration_checkpoint(point: &str) -> Result<(), String> {
    if std::env::var("OMEGAT_TEST_LEGACY_MIGRATION_POINT").as_deref() != Ok(point) {
        return Ok(());
    }
    let Some(marker) = std::env::var_os("OMEGAT_TEST_LEGACY_MIGRATION_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create migration marker directory: {error}"))?;
    }
    let mut file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(format!("create migration marker: {error}")),
    };
    writeln!(file, "{point}").map_err(|error| format!("write migration marker: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("sync migration marker: {error}"))?;
    if let Some(parent) = marker.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("sync migration marker directory: {error}"))?;
    }
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn read_legacy_history(root: &Path) -> Result<Vec<RefreshEnvelope>, String> {
    let path = legacy_history_path(root);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("read refresh history {}: {error}", path.display())),
    };
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                format!(
                    "parse refresh history {} line {}: {error}",
                    path.display(),
                    index + 1
                )
            })
        })
        .collect()
}

fn migrate_legacy_journal(root: &Path) -> Result<(), String> {
    let journal_path = legacy_journal_path(root);
    let history_path = legacy_history_path(root);
    let journal = read_json::<LegacyRefreshJournal>(&journal_path)?;
    if journal.is_none() && !history_path.is_file() {
        return Ok(());
    }
    let active = if let Some(journal) = journal {
        if journal.version != LEGACY_QUEUE_VERSION {
            return Err(format!(
                "unsupported refresh journal version {}",
                journal.version
            ));
        }
        if normalized(&journal.project_root) != normalized(root) {
            return Err(format!(
                "refresh journal root {} does not match {}",
                journal.project_root.display(),
                root.display()
            ));
        }
        journal.batches
    } else {
        Vec::new()
    };
    let history = read_legacy_history(root)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh migration: {error}"))?;
    omegat_team::migrate_refresh_transactions(&props, active, history)
        .map_err(|error| format!("migrate refresh journal: {error}"))?;
    // The shared queue and history are durable before either legacy source is
    // removed. A crash between removals merely repeats exact-id migration.
    legacy_migration_checkpoint("after_shared_journal_publish")?;
    remove_file(&journal_path)?;
    remove_file(&history_path)
}

fn write_active(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
) -> Result<(), String> {
    write_json(
        &active_path(config_dir, app_instance, root),
        &ActiveProject {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: normalized(root),
            app_instance: app_instance.to_string(),
            generation,
            updated_unix_ms: unix_ms(),
        },
    )
}

fn migrate_legacy_active(config_dir: &Path) -> Result<(), String> {
    let legacy_path = legacy_active_path(config_dir);
    let Some(active) = read_json::<ActiveProject>(&legacy_path)? else {
        return Ok(());
    };
    if active.version != TRANSACTION_ENVELOPE_VERSION {
        return Err(format!(
            "unsupported active refresh owner version {}",
            active.version
        ));
    }
    write_json(
        &active_path(config_dir, &active.app_instance, &active.project_root),
        &active,
    )?;
    legacy_migration_checkpoint("after_config_owner_publish")?;
    remove_file(&legacy_path)
}

fn migrate_per_app_active(
    config_dir: &Path,
    app_instance: &str,
    selected_root: &Path,
) -> Result<(), String> {
    let legacy = legacy_per_app_active_path(config_dir, app_instance);
    let destination = active_path(config_dir, app_instance, selected_root);
    if legacy == destination || !legacy.is_file() {
        return Ok(());
    }
    let Some(active) = read_json::<ActiveProject>(&legacy)? else {
        return Ok(());
    };
    if active.version != TRANSACTION_ENVELOPE_VERSION || active.app_instance != app_instance {
        return Err(format!(
            "invalid legacy per-app refresh owner {}",
            legacy.display()
        ));
    }
    let actual_destination = active_path(config_dir, app_instance, &active.project_root);
    if let Some(existing) = read_json::<ActiveProject>(&actual_destination)? {
        if existing.project_root != active.project_root
            || existing.app_instance != active.app_instance
            || existing.generation != active.generation
        {
            return Err(format!(
                "legacy per-app refresh owner conflicts with {}",
                actual_destination.display()
            ));
        }
    } else {
        write_json(&actual_destination, &active)?;
    }
    legacy_migration_checkpoint("after_per_root_owner_publish")?;
    remove_file(&legacy)
}

fn discard_root_refreshes(root: &Path) -> Result<(), String> {
    // A config-scoped owner pointer can outlive a project that was moved or
    // deleted while OmegaT was not running. There is no durable project queue
    // left to discard in that case, and the caller will atomically replace the
    // stale owner pointer with the newly selected root.
    if !root.join("omegat.project").is_file() {
        return Ok(());
    }
    migrate_legacy_journal(root)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh discard: {error}"))?;
    omegat_team::discard_refresh_transactions(&props)
        .map_err(|error| format!("discard refresh transactions: {error}"))
}

fn select_active_project(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
) -> Result<(), String> {
    migrate_legacy_active(config_dir)?;
    migrate_per_app_active(config_dir, app_instance, root)?;
    migrate_legacy_journal(root)?;
    let owner_path = active_path(config_dir, app_instance, root);
    if let Some(active) = read_json::<ActiveProject>(&owner_path)? {
        if active.version != TRANSACTION_ENVELOPE_VERSION {
            return Err(format!(
                "unsupported active refresh owner version {}",
                active.version
            ));
        }
        if active.app_instance != app_instance {
            return Err(format!("active refresh owner collision for {app_instance}"));
        }
        if normalized(&active.project_root) != normalized(root) {
            return Err(format!(
                "per-root refresh owner {} points at a different project",
                owner_path.display()
            ));
        }
        if active.generation != 0 && active.generation != generation {
            // Product/team receipts remain untouched. Only stale filesystem
            // work for this exact project owner is made terminal.
            discard_root_refreshes(root)?;
        }
    }
    write_active(config_dir, root, app_instance, generation)
}

/// Prepare config ownership and migrate any former refresh-only journal.
pub fn prepare(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
) -> Result<(), String> {
    if app_instance.is_empty() || generation == 0 {
        return Err("refresh prepare requires app instance and generation".into());
    }
    select_active_project(config_dir, root, app_instance, generation)
}

/// Return roots recorded by config-scoped Electron owners without adopting a queue.
pub fn active_project_roots(config_dir: &Path) -> Result<Vec<PathBuf>, String> {
    migrate_legacy_active(config_dir)?;
    let directory = config_dir.join("transactions").join(ACTIVE_DIRECTORY);
    let roots = discover_scopes::<ActiveProject>(&directory)?;
    for root in &roots {
        migrate_legacy_journal(root)?;
    }
    Ok(roots)
}

pub fn enqueue(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    paths: Vec<String>,
    fingerprints: BTreeMap<String, Option<String>>,
    sources: Vec<String>,
) -> Result<TransactionRendererReceipt, String> {
    prepare(config_dir, root, app_instance, generation)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh enqueue: {error}"))?;
    let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let batch_id = format!("refresh-{}-{}-{sequence}", unix_ms(), std::process::id());
    omegat_team::enqueue_refresh_transaction(
        &props,
        generation,
        &batch_id,
        paths,
        fingerprints,
        sources,
    )
    .map_err(|error| error.to_string())
}

pub fn checkpoint_sidecar_commit(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
    committed_result: &serde_json::Value,
) -> Result<TransactionRendererReceipt, String> {
    prepare(config_dir, root, app_instance, generation)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh checkpoint: {error}"))?;
    omegat_team::checkpoint_refresh_transaction(&props, generation, batch_id, committed_result)
        .map_err(|error| error.to_string())
}

pub fn request_cancelled(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
) -> Result<(), String> {
    prepare(config_dir, root, app_instance, generation)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh cancellation: {error}"))?;
    omegat_team::cancel_refresh_transaction(&props, generation, batch_id)
        .map_err(|error| error.to_string())
}

pub fn acknowledge(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
    batch_id: &str,
    outcome: &str,
) -> Result<TransactionRendererAck, String> {
    prepare(config_dir, root, app_instance, generation)?;
    let props = omegat_core::properties::ProjectProperties::load(root)
        .map_err(|error| format!("load project for refresh acknowledgement: {error}"))?;
    omegat_team::acknowledge_transaction_receipt_outcome(
        &props,
        generation,
        batch_id,
        "project.external-refresh",
        outcome,
    )
    .map_err(|error| error.to_string())
}

pub fn discard(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    migrate_legacy_journal(root)?;
    discard_root_refreshes(root)?;
    migrate_legacy_active(config_dir)?;
    migrate_per_app_active(config_dir, app_instance, root)?;
    let owner_path = active_path(config_dir, app_instance, root);
    if let Some(active) = read_json::<ActiveProject>(&owner_path)? {
        if normalized(&active.project_root) == normalized(root)
            && active.app_instance == app_instance
        {
            remove_file(&owner_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omegat_core::properties::ProjectProperties;
    use omegat_team::{TransactionStatus, REQUEST_CANCELLED_CODE};

    fn project(root: &Path) -> ProjectProperties {
        let props = ProjectProperties::create(root.to_path_buf(), "en".into(), "fr".into(), false);
        props.ensure_dirs().unwrap();
        props.write().unwrap();
        props
    }

    fn fingerprints(value: &str) -> BTreeMap<String, Option<String>> {
        BTreeMap::from([("source/a.txt".into(), Some(value.into()))])
    }

    #[test]
    fn owner_roots_are_independent_and_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        project(&first);
        project(&second);
        write_active(&config, &first, "electron-a", 1).unwrap();
        write_active(&config, &first, "electron-b", 2).unwrap();
        write_active(&config, &second, "electron-c", 3).unwrap();

        let roots = active_project_roots(&config).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&normalized(&first)));
        assert!(roots.contains(&normalized(&second)));
    }

    #[test]
    fn one_app_can_rotate_across_roots_without_discarding_other_root_owner() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let deleted = temp.path().join("deleted");
        let current = temp.path().join("current");
        project(&deleted);
        project(&current);
        write_active(&config, &deleted, "electron", 4).unwrap();
        std::fs::remove_dir_all(&deleted).unwrap();

        prepare(&config, &current, "electron", 5).unwrap();

        let active = read_json::<ActiveProject>(&active_path(&config, "electron", &current))
            .unwrap()
            .unwrap();
        assert_eq!(active.project_root, normalized(&current));
        assert_eq!(active.generation, 5);
        assert!(active_path(&config, "electron", &deleted).is_file());
    }

    #[test]
    fn refresh_uses_shared_active_queue_and_protocol_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join("project");
        project(&root);
        let batch = enqueue(
            &config,
            &root,
            "electron",
            7,
            vec!["source/a.txt".into()],
            fingerprints("one"),
            vec!["native".into()],
        )
        .unwrap();
        assert!(!legacy_journal_path(&root).exists());
        assert!(transaction_dir(&root).join("active.json").exists());
        assert_eq!(batch.status, TransactionStatus::Pending);

        request_cancelled(&config, &root, "electron", 7, &batch.batch_id).unwrap();
        let history =
            std::fs::read_to_string(transaction_dir(&root).join("history.ndjson")).unwrap();
        assert!(history.contains(&batch.batch_id));
        assert!(history.contains(&REQUEST_CANCELLED_CODE.to_string()));
    }

    #[test]
    fn interrupted_legacy_migration_is_idempotent_across_project_and_config_scopes() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let root = temp.path().join("project");
        let props = project(&root);
        let payload = TransactionRendererPayload {
            operation: "project.external-refresh".into(),
            paths: vec!["source/a.txt".into()],
            fingerprints: fingerprints("legacy"),
            sources: vec!["native".into()],
            committed_result: None,
        };
        let pending = TransactionEnvelope::pending(&root, 7, "legacy-pending", payload.clone());
        let mut completed = TransactionEnvelope::pending(&root, 6, "legacy-completed", payload);
        completed.transition(TransactionStatus::Completed, None);
        let journal = LegacyRefreshJournal {
            version: LEGACY_QUEUE_VERSION,
            project_root: root.clone(),
            app_instance: "legacy-electron".into(),
            generation: 7,
            batches: vec![pending.clone()],
            updated_unix_ms: 1,
        };
        write_json(&legacy_journal_path(&root), &journal).unwrap();
        std::fs::write(
            legacy_history_path(&root),
            format!("{}\n", serde_json::to_string(&completed).unwrap()),
        )
        .unwrap();
        let active_owner = ActiveProject {
            version: TRANSACTION_ENVELOPE_VERSION,
            project_root: root.clone(),
            app_instance: "legacy-electron".into(),
            generation: 7,
            updated_unix_ms: 2,
        };
        write_json(&legacy_active_path(&config), &active_owner).unwrap();

        // Simulate process death after each destination became durable but
        // before its legacy source was unlinked.
        omegat_team::migrate_refresh_transactions(&props, vec![pending], vec![completed]).unwrap();
        write_json(
            &active_path(&config, "legacy-electron", &root),
            &active_owner,
        )
        .unwrap();
        assert!(legacy_journal_path(&root).is_file());
        assert!(legacy_history_path(&root).is_file());
        assert!(legacy_active_path(&config).is_file());

        migrate_legacy_journal(&root).unwrap();
        migrate_legacy_active(&config).unwrap();
        assert!(!legacy_journal_path(&root).exists());
        assert!(!legacy_history_path(&root).exists());
        assert!(!legacy_active_path(&config).exists());
        assert!(active_path(&config, "legacy-electron", &root).is_file());

        let active: serde_json::Value = serde_json::from_slice(
            &std::fs::read(transaction_dir(&root).join("active.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(active["batches"].as_array().unwrap().len(), 1);
        assert_eq!(active["batches"][0]["batch_id"], "legacy-pending");
        let history =
            std::fs::read_to_string(transaction_dir(&root).join("history.ndjson")).unwrap();
        assert_eq!(
            history
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter(|row| row["batch_id"] == "legacy-completed")
                .count(),
            1
        );
    }
}
