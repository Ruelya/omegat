// SPDX-License-Identifier: GPL-3.0-or-later

//! Compatibility exports for the core durable transaction envelope.
//!
//! Product/team payloads remain in `omegat-team`; the storage and state-machine
//! contract intentionally lives in `omegat-core` so config and project
//! workflows cannot diverge.

pub use omegat_core::durable_transaction::{
    normalized, write_json_atomic, TransactionCommit, TransactionEnvelope, TransactionStatus,
    REQUEST_CANCELLED_CODE, TRANSACTION_ENVELOPE_VERSION,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_envelope_covers_terminal_and_checkpoint_states() {
        let temp = tempfile::tempdir().unwrap();
        let mut envelope = TransactionEnvelope::pending(
            temp.path(),
            7,
            "batch-7",
            serde_json::json!({"kind": "external-refresh"}),
        );
        envelope.validate_for_root(temp.path()).unwrap();
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
        envelope.validate_for_root(temp.path()).unwrap();
        assert_eq!(
            serde_json::to_value(envelope).unwrap()["status"],
            "request_cancelled"
        );
    }

    #[test]
    fn core_envelope_rejects_unknown_fields_and_future_versions() {
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
        assert!(
            serde_json::from_value::<TransactionEnvelope<serde_json::Value>>(unknown).is_err()
        );
        let future: TransactionEnvelope<serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "version": 2,
                "project_root": root,
                "generation": 1,
                "batch_id": "future-v2",
                "status": "completed",
                "error_code": null,
                "updated_unix_ms": 1,
                "payload": {}
            }))
            .unwrap();
        assert_eq!(
            future.validate_for_root(temp.path()).unwrap_err(),
            "unsupported transaction envelope version 2"
        );
    }

    #[test]
    fn core_atomic_publish_keeps_receipt_with_terminal_status() {
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
