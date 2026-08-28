// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared durable transaction identity and terminal-state contract.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TRANSACTION_ENVELOPE_VERSION: u8 = 1;
pub const REQUEST_CANCELLED_CODE: i32 = -32800;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    SidecarCommitted,
    Completed,
    Cancelled,
    RequestCancelled,
}

impl TransactionStatus {
    pub fn is_recoverable(self) -> bool {
        matches!(self, Self::Pending | Self::SidecarCommitted)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TransactionEnvelope<T> {
    pub version: u8,
    pub project_root: PathBuf,
    pub generation: u64,
    pub batch_id: String,
    pub status: TransactionStatus,
    pub error_code: Option<i32>,
    pub updated_unix_ms: u128,
    pub payload: T,
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
        if self.status == TransactionStatus::RequestCancelled
            && self.error_code != Some(REQUEST_CANCELLED_CODE)
        {
            return Err("request-cancelled transaction envelope must carry -32800".into());
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
        self.touch();
    }

    pub fn touch(&mut self) {
        self.updated_unix_ms = unix_ms();
    }
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
        envelope.transition(TransactionStatus::SidecarCommitted, None);
        assert!(envelope.status.is_recoverable());
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
    }
}
