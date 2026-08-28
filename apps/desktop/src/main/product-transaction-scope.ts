// SPDX-License-Identifier: GPL-3.0-or-later

const PRODUCT_TRANSACTION_METHODS = new Set([
  "entry.set",
  "project.save",
  "project.close",
  "project.reload",
  "project.compile",
  "project.import",
  "project.update",
  "team.mapping",
  "align.run",
  "align.write",
]);

export function scopeProductTransaction(
  method: string,
  params: unknown,
  project: { root: string; generation: number } | null,
  createBatchId: () => string,
): unknown {
  if (!project || !PRODUCT_TRANSACTION_METHODS.has(method)) return params;
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
 * acknowledgement. The in-memory ownership set disappears with Electron, so
 * process restart still republishes every durable unacknowledged receipt.
 */
export function transactionEnvelopesForRenderer<T extends TransactionReceiptIdentity>(
  envelopes: readonly T[],
  callerManagedReceipts: ReadonlySet<string>,
): T[] {
  return envelopes.filter((envelope) =>
    !callerManagedReceipts.has(transactionReceiptIdentity(envelope))
  );
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
