// SPDX-License-Identifier: GPL-3.0-or-later

use omegat_ipc::RpcNotification;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::{Duration, SystemTime};

const WATCHED_PROJECT_DIRS: &[&str] = &["source", "omegat", "tm", "glossary", "dictionary"];
const SCAN_INTERVAL: Duration = Duration::from_millis(75);

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified_nanos: u128,
}

pub enum WatchCommand {
    Watch(PathBuf, SyncSender<()>),
    Close,
    Shutdown,
}

pub fn spawn(output: Sender<String>) -> (Sender<WatchCommand>, thread::JoinHandle<()>) {
    let (commands, command_rx) = mpsc::channel();
    let worker = thread::spawn(move || run(command_rx, output));
    (commands, worker)
}

fn run(commands: Receiver<WatchCommand>, output: Sender<String>) {
    let mut root: Option<PathBuf> = None;
    let mut snapshot = BTreeMap::new();
    loop {
        match commands.recv_timeout(SCAN_INTERVAL) {
            Ok(WatchCommand::Watch(next_root, ready)) => {
                snapshot = scan_project(&next_root);
                root = Some(next_root);
                let _ = ready.send(());
                continue;
            }
            Ok(WatchCommand::Close) => {
                root = None;
                snapshot.clear();
                continue;
            }
            Ok(WatchCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let Some(active_root) = root.as_ref() else {
            continue;
        };
        let next = scan_project(active_root);
        let mut changed = Vec::new();
        for (path, fingerprint) in &next {
            if snapshot.get(path) != Some(fingerprint) {
                changed.push(path.clone());
            }
        }
        for path in snapshot.keys() {
            if !next.contains_key(path) {
                changed.push(path.clone());
            }
        }
        snapshot = next;
        changed.sort();
        changed.dedup();
        if changed.is_empty() {
            continue;
        }
        let notification = RpcNotification::new(
            "project.files-changed",
            json!({
                "root": active_root.to_string_lossy(),
                "paths": changed
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>(),
            }),
        );
        if let Ok(line) = serde_json::to_string(&notification) {
            let _ = output.send(line);
        }
    }
}

fn scan_project(root: &Path) -> BTreeMap<PathBuf, FileFingerprint> {
    let mut files = BTreeMap::new();
    record_file(&root.join("omegat.project"), &mut files);
    for directory in WATCHED_PROJECT_DIRS {
        collect_files(&root.join(directory), &mut files);
    }
    files
}

fn collect_files(path: &Path, files: &mut BTreeMap<PathBuf, FileFingerprint>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, files);
        } else if file_type.is_file() {
            record_file(&path, files);
        }
    }
}

fn record_file(path: &Path, files: &mut BTreeMap<PathBuf, FileFingerprint>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    files.insert(
        path.to_path_buf(),
        FileFingerprint {
            len: metadata.len(),
            modified_nanos,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_runtime_nested_file_events_on_the_ndjson_channel() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        std::fs::create_dir_all(root.join("source")).unwrap();
        let (output, notifications) = mpsc::channel();
        let (commands, watcher) = spawn(output);
        let (ready, ready_rx) = mpsc::sync_channel(0);
        commands
            .send(WatchCommand::Watch(root.clone(), ready))
            .unwrap();
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let nested = root.join("source/runtime/new");
        std::fs::create_dir_all(&nested).unwrap();
        let input = nested.join("chapter.txt");
        std::fs::write(&input, "runtime source").unwrap();
        let line = notifications.recv_timeout(Duration::from_secs(2)).unwrap();
        let notification: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            notification,
            json!({
                "jsonrpc": "2.0",
                "method": "project.files-changed",
                "params": {
                    "root": root.to_string_lossy(),
                    "paths": [input.to_string_lossy()]
                }
            })
        );

        commands.send(WatchCommand::Shutdown).unwrap();
        watcher.join().unwrap();
    }
}
