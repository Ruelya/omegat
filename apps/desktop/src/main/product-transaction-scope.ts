// SPDX-License-Identifier: GPL-3.0-or-later

import { isProjectTransactionMethod } from "../shared/writer-catalog";

export function scopeProductTransaction(
  method: string,
  params: unknown,
  project: { root: string; generation: number } | null,
  createBatchId: () => string,
): unknown {
  if (!project || !isProjectTransactionMethod(method)) return params;
  const input = params !== null && typeof params === "object" ? params : {};
  return {
    ...input,
    transaction_project_root: project.root,
    transaction_generation: project.generation,
    transaction_batch_id: `product-${method}-${createBatchId()}`,
  };
}

type TransactionReceiptIdentity = {
  project_root: string;
  generation: number;
  batch_id: string;
  payload: { operation: string };
};

/**
 * A caller-managed receipt belongs to its operation-specific renderer action.
 * Publishing it on the recovery channel races that action's state update and
 * acknowledgement. A recovery receipt already sent to the current renderer
 * must likewise stay single-delivery while concurrent recovery triggers query
 * the same durable head. Both in-memory sets disappear with Electron (and are
 * cleared on renderer reload), so restart still republishes every durable
 * unacknowledged receipt.
 */
export function transactionEnvelopesForRenderer<T extends TransactionReceiptIdentity>(
  envelopes: readonly T[],
  callerManagedReceipts: ReadonlySet<string>,
  publishedRecoveryReceipts: ReadonlySet<string> = new Set(),
): T[] {
  return envelopes.filter((envelope) => {
    const identity = transactionReceiptIdentity(envelope);
    return (
      !callerManagedReceipts.has(identity)
      && !publishedRecoveryReceipts.has(identity)
    );
  });
}

export function transactionReceiptIdentity(
  receipt: TransactionReceiptIdentity,
): string {
  return JSON.stringify([
    receipt.project_root,
    receipt.generation,
    receipt.batch_id,
    receipt.payload.operation,
  ]);
}
