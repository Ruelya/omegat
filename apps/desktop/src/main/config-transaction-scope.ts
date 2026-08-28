// SPDX-License-Identifier: GPL-3.0-or-later

import { isConfigTransactionMethodFromCatalog } from "../shared/writer-catalog";

export function isConfigTransactionMethod(method: string, params: unknown): boolean {
  return isConfigTransactionMethodFromCatalog(method, params);
}

export function scopeConfigTransaction(
  method: string,
  params: unknown,
  appInstance: string,
  ownerProcessId: number,
  createBatchId: () => string,
): unknown {
  if (!isConfigTransactionMethod(method, params)) return params;
  const input: Record<string, unknown> =
    params !== null && typeof params === "object"
      ? params as Record<string, unknown>
      : {};
  const retryBatchId = "config_transaction_retry_batch_id" in input
    && typeof input.config_transaction_retry_batch_id === "string"
    && input.config_transaction_retry_batch_id.length > 0
    ? input.config_transaction_retry_batch_id
    : null;
  const payload = { ...input };
  delete payload.config_transaction_retry_batch_id;
  return {
    ...payload,
    config_transaction_app_instance: appInstance,
    config_transaction_batch_id:
      retryBatchId ?? `config-${method}-${createBatchId()}`,
    config_transaction_owner_process_id: ownerProcessId,
  };
}
