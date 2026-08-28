// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared durable transaction identity and terminal-state contract.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TRANSACTION_ENVELOPE_VERSION: u8 = 1;
pub const REQUEST_CANCELLED_CODE: i32 = -32800;
static ATOMIC_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

/// Fingerprint of the product result published by the transaction.
///
/// The receipt lives inside the same atomically-renamed envelope as its
/// committed status. A recoverer therefore sees either the prior pending
/// envelope (and rolls product writes back) or the committed receipt (and
/// finalizes them), never a committed status without its exact result.
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
            project_root: normalized(project_root),
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
        if normalized(&self.project_root) != normalized(root) {
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

/// Durably replace one JSON state file with a single fsync/rename transaction.
///
/// Callers put both their product commit receipt and envelope status in
/// `value`. The parent directory sync closes the rename durability gap.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("transaction state has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create transaction directory {}: {error}", parent.display()))?;
    let sequence = ATOMIC_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("transaction");
    let temporary = parent.join(format!(".{filename}.{}.{sequence}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize transaction state: {error}"))?;
    let write_result = {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "create transaction temporary {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| {
                format!(
                    "write transaction temporary {}: {error}",
                    temporary.display()
                )
            })
    };
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "publish transaction state {}: {error}",
            path.display()
        ));
    }
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync transaction directory {}: {error}", parent.display()))
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

    #[test]
    fn terminal_and_checkpoint_states_share_one_versioned_shape() {
        let temp = tempfile::tempdir().unwrap();
        let mut envelope = TransactionEnvelope::pending(
            temp.path(),
            7,
            "batch-7",
            serde_json::json!({"kind": "external-refresh"}),
        );
        envelope
            .validate_for_root(temp.path())
            .expect("pending envelope");
        let manifest = serde_json::json!({"entries": ["one", "two"]});
        envelope
            .commit_product(TransactionStatus::SidecarCommitted, &manifest, 2)
            .unwrap();
        assert!(envelope.status.is_recoverable());
        assert!(envelope.verify_product(&manifest, 2));
        envelope.transition(
            TransactionStatus::RequestCancelled,
            Some(REQUEST_CANCELLED_CODE),
        );
        assert!(!envelope.status.is_recoverable());
        envelope
            .validate_for_root(temp.path())
            .expect("cancelled envelope");

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["version"], TRANSACTION_ENVELOPE_VERSION);
        assert_eq!(json["generation"], 7);
        assert_eq!(json["batch_id"], "batch-7");
        assert_eq!(json["status"], "request_cancelled");
        assert_eq!(json["error_code"], REQUEST_CANCELLED_CODE);
        assert_eq!(json.get("commit"), None);
    }

    #[test]
    fn rejects_unknown_v1_fields_and_future_versions_without_reviving_terminal_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = normalized(temp.path());
        let unknown = serde_json::json!({
            "version": 1,
            "project_root": root.clone(),
            "generation": 1,
            "batch_id": "unknown-v1",
            "status": "completed",
            "error_code": null,
            "updated_unix_ms": 1,
            "payload": {},
            "future_state": "pending"
        });
        assert!(serde_json::from_value::<TransactionEnvelope<serde_json::Value>>(unknown).is_err());

        let future = serde_json::json!({
            "version": 2,
            "project_root": root,
            "generation": 1,
            "batch_id": "future-v2",
            "status": "completed",
            "error_code": null,
            "updated_unix_ms": 1,
            "payload": {}
        });
        let future: TransactionEnvelope<serde_json::Value> =
            serde_json::from_value(future).unwrap();
        assert!(!future.status.is_recoverable());
        assert_eq!(
            future.validate_for_root(temp.path()).unwrap_err(),
            "unsupported transaction envelope version 2"
        );
    }

    #[test]
    fn atomic_json_publish_never_exposes_a_partial_commit_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("active.json");
        let mut envelope = TransactionEnvelope::pending(
            temp.path(),
            1,
            "atomic",
            serde_json::json!({"operation": "test"}),
        );
        write_json_atomic(&path, &envelope).unwrap();
        envelope
            .commit_product(
                TransactionStatus::Completed,
                &serde_json::json!({"result": "durable"}),
                1,
            )
            .unwrap();
        write_json_atomic(&path, &envelope).unwrap();

        let loaded: TransactionEnvelope<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(loaded.status, TransactionStatus::Completed);
        assert!(loaded.commit.is_some());
    }
}
