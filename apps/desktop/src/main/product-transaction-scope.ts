// SPDX-License-Identifier: GPL-3.0-or-later

const PRODUCT_TRANSACTION_METHODS = new Set([
  "entry.set",
  "project.save",
  "project.close",
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
