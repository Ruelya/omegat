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
  batch_id: string;
  payload: { operation: string };
};

/**
 * A caller-managed receipt returned by the current RPC belongs to that RPC's
 * renderer action. Publishing the same receipt on the recovery channel races
 * its operation-specific state update and acknowledgement. Older FIFO heads,
 * restart recovery, and raw bridge callers still use the channel.
 */
export function transactionEnvelopesForRenderer<T extends TransactionReceiptIdentity>(
  envelopes: readonly T[],
  directlyReturnedReceipt: { batchId: string; operation: string } | null | undefined,
  callerManagesReceipt: boolean,
): T[] {
  if (!directlyReturnedReceipt || !callerManagesReceipt) return [...envelopes];
  return envelopes.filter((envelope) =>
    envelope.batch_id !== directlyReturnedReceipt.batchId
    || envelope.payload.operation !== directlyReturnedReceipt.operation
  );
}
