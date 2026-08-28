// SPDX-License-Identifier: GPL-3.0-or-later

export type TransactionStatus =
  | "pending"
  | "cancellation_pending"
  | "sidecar_committed"
  | "completed"
  | "cancelled"
  | "request_cancelled";

export type TransactionEnvelopePayload = {
  operation: string;
  paths?: string[];
  fingerprints?: Record<string, string | null>;
  sources?: Array<"native" | "sidecar">;
  committed_result?: {
    entry_list: unknown[];
    props: unknown;
    stats: unknown;
  };
};

export type TransactionEnvelope = {
  version: number;
  project_root: string;
  generation: number;
  batch_id: string;
  status: TransactionStatus;
  error_code: number | null;
  updated_unix_ms: number;
  payload: TransactionEnvelopePayload;
  commit?: {
    manifest_sha256: string;
    manifest_items: number;
  } | null;
};

export type TransactionAck = {
  version: number;
  project_root: string;
  generation: number;
  batch_id: string;
  acknowledged: boolean;
  already_acknowledged: boolean;
};

export type TransactionOutcome = "succeeded" | "cancelled" | "coalesced";

const CALLER_MANAGED_TRANSACTION_METHODS = new Set([
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
  "align.run",
  "align.write",
]);

export function isCallerManagedTransactionMethod(method: string): boolean {
  return CALLER_MANAGED_TRANSACTION_METHODS.has(method);
}
