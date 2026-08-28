// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable FIFO for filesystem fingerprints awaiting an external refresh.
//!
//! The journal deliberately lives beside the team transaction journal under
//! `.repositories/transactions`, while a config-scoped pointer identifies the
//! one project that was active in the Electron application.  This gives both
//! recovery paths the same project-root and generation rules without making
//! `omegat-core` depend on `omegat-team`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FORMAT: u8 = 1;
const JOURNAL_FILE: &str = "external-refresh.json";
const ACTIVE_FILE: &str = "external-refresh-active.json";
static BATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RefreshBatch {
    pub id: String,
    pub paths: Vec<String>,
    pub fingerprints: BTreeMap<String, Option<String>>,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RefreshJournal {
    format: u8,
    project_root: PathBuf,
    app_instance: String,
    generation: u64,
    batches: Vec<RefreshBatch>,
    updated_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActiveProject {
    format: u8,
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
    let parent = path
        .parent()
        .ok_or_else(|| format!("refresh journal has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create refresh journal {}: {error}", parent.display()))?;
    let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{JOURNAL_FILE}.{}.{sequence}.tmp",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize refresh journal: {error}"))?;
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "create refresh journal temporary {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| {
                format!(
                    "write refresh journal temporary {}: {error}",
                    temporary.display()
                )
            })?;
    }
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish refresh journal {}: {error}", path.display()))?;
    sync_parent(path)
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

fn load_journal(root: &Path) -> Result<Option<RefreshJournal>, String> {
    let Some(journal) = read_json::<RefreshJournal>(&journal_path(root))? else {
        return Ok(None);
    };
    if journal.format != FORMAT {
        return Err(format!(
            "unsupported refresh journal format {}",
            journal.format
        ));
    }
    if normalized(&journal.project_root) != normalized(root) {
        remove_file(&journal_path(root))?;
        return Ok(None);
    }
    Ok(Some(journal))
}

fn write_active(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    write_json(
        &active_path(config_dir),
        &ActiveProject {
            format: FORMAT,
            project_root: normalized(root),
            app_instance: app_instance.to_string(),
            updated_unix_ms: unix_ms(),
        },
    )
}

fn select_active_project(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    if let Some(active) = read_json::<ActiveProject>(&active_path(config_dir))? {
        if active.format != FORMAT {
            return Err(format!(
                "unsupported active refresh journal format {}",
                active.format
            ));
        }
        if normalized(&active.project_root) != normalized(root) {
            // Opening a different root is a project-generation boundary.  A
            // batch from the formerly active root must never be replayed when
            // that project happens to be opened again later.
            remove_file(&journal_path(&active.project_root))?;
        }
    }
    write_active(config_dir, root, app_instance)
}

pub fn pending(
    config_dir: &Path,
    root: &Path,
    app_instance: &str,
    generation: u64,
) -> Result<Vec<RefreshBatch>, String> {
    select_active_project(config_dir, root, app_instance)?;
    let Some(mut journal) = load_journal(root)? else {
        return Ok(Vec::new());
    };
    if journal.app_instance == app_instance && journal.generation != generation {
        // The same Electron process advanced its project generation.  This is
        // a reload/open boundary, not crash recovery.
        remove_file(&journal_path(root))?;
        return Ok(Vec::new());
    }
    if journal.app_instance != app_instance {
        // A new Electron process may adopt only the queue for the same active
        // project root.  Re-stamp its renderer generation before replay.
        journal.app_instance = app_instance.to_string();
        journal.generation = generation;
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
) -> Result<RefreshBatch, String> {
    let _ = pending(config_dir, root, app_instance, generation)?;
    let mut journal = load_journal(root)?.unwrap_or_else(|| RefreshJournal {
        format: FORMAT,
        project_root: normalized(root),
        app_instance: app_instance.to_string(),
        generation,
        batches: Vec::new(),
        updated_unix_ms: unix_ms(),
    });
    if let Some(existing) = journal
        .batches
        .iter_mut()
        .find(|batch| batch.fingerprints == fingerprints)
    {
        for source in sources {
            if !existing.sources.contains(&source) {
                existing.sources.push(source);
            }
        }
        existing.sources.sort();
        let result = existing.clone();
        journal.updated_unix_ms = unix_ms();
        write_json(&journal_path(root), &journal)?;
        return Ok(result);
    }
    let sequence = BATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let batch = RefreshBatch {
        id: format!("refresh-{}-{}-{sequence}", unix_ms(), std::process::id()),
        paths,
        fingerprints,
        sources,
    };
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
) -> Result<Vec<RefreshBatch>, String> {
    let pending = pending(config_dir, root, app_instance, generation)?;
    let Some(first) = pending.first() else {
        return Ok(Vec::new());
    };
    if first.id != batch_id {
        return Err(format!("refresh FIFO head is {}, not {batch_id}", first.id));
    }
    let mut journal = load_journal(root)?
        .ok_or_else(|| "refresh journal disappeared before completion".to_string())?;
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

pub fn discard(config_dir: &Path, root: &Path, app_instance: &str) -> Result<(), String> {
    remove_file(&journal_path(root))?;
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
        assert_eq!(
            pending(&config, &first, "electron-two", 1).unwrap(),
            vec![one.clone(), two.clone()]
        );
        assert_eq!(
            complete(&config, &first, "electron-two", 1, &one.id).unwrap(),
            vec![two]
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
        assert!(complete(&config, &root, "electron", 4, &batch.id)
            .unwrap()
            .is_empty());
        assert!(pending(&config, &root, "electron", 4).unwrap().is_empty());

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
