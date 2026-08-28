// SPDX-License-Identifier: GPL-3.0-or-later

import {
  isLongOperationMethod,
  type RpcOperationEvent,
} from "../shared/rpc-operation";

type PendingRequest = {
  clientRequestId: string | null;
  cancellationSourceId: string | null;
  method: string;
  deferTerminal: boolean;
  cancellationRequested: boolean;
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
};

type RpcResponse = {
  id?: number;
  result?: unknown;
  error?: { code?: number; message?: string };
  method?: string;
  params?: unknown;
};

export class SidecarRpcClient {
  private readonly pending = new Map<number, PendingRequest>();
  private readonly clientRequests = new Map<string, number>();
  private readonly deferred = new Map<
    string,
    { method: string; cancellationRequested: boolean }
  >();
  private nextId = 1;
  private buffer = "";

  constructor(
    private readonly writeLine: (line: string) => void,
    private readonly onNotification: (method: string, params: unknown) => void = () => undefined,
    private readonly onOperation: (event: RpcOperationEvent) => void = () => undefined,
  ) {}

  request(
    method: string,
    params: unknown = {},
    clientRequestId: string | null = null,
    deferTerminal = false,
    cancellationSourceId: string | null = null,
  ): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const request = {
        clientRequestId,
        cancellationSourceId,
        method,
        deferTerminal,
        cancellationRequested: false,
        resolve,
        reject,
      };
      this.pending.set(id, request);
      if (clientRequestId) this.clientRequests.set(clientRequestId, id);
      this.writeLine(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
      if (clientRequestId) {
        this.onOperation({
          requestId: clientRequestId,
          method,
          phase: "started",
        });
      }
      if (
        cancellationSourceId
        && this.deferred.get(cancellationSourceId)?.cancellationRequested
      ) {
        this.requestCancellation(id, request);
      }
    });
  }

  /**
   * Cancel a renderer request on the NDJSON wire, not only in the publishing
   * component. The sidecar maps this notification to its cooperative token.
   */
  cancel(clientRequestId: string): boolean {
    const id = this.clientRequests.get(clientRequestId);
    if (id === undefined) {
      const deferred = this.deferred.get(clientRequestId);
      if (!deferred) return false;
      if (deferred.cancellationRequested) return true;
      deferred.cancellationRequested = true;
      this.onOperation({
        requestId: clientRequestId,
        method: deferred.method,
        phase: "cancelling",
      });
      for (const [pendingId, pending] of this.pending) {
        if (pending.cancellationSourceId === clientRequestId) {
          this.requestCancellation(pendingId, pending);
        }
      }
      return true;
    }
    const request = this.pending.get(id);
    if (!request) return false;
    if (request.cancellationRequested) return true;
    request.cancellationRequested = true;
    this.onOperation({
      requestId: clientRequestId,
      method: request.method,
      phase: "cancelling",
    });
    this.writeLine(JSON.stringify({
      jsonrpc: "2.0",
      method: "$/cancelRequest",
      params: { id },
    }));
    // Long operations remain pending until the sidecar acknowledges the
    // cooperative token with -32800. This keeps "cancelling" visibly distinct
    // from "cancelled" and prevents a local IPC write from claiming rollback
    // before product work has actually stopped.
    if (isLongOperationMethod(request.method)) {
      return true;
    }
    this.clientRequests.delete(clientRequestId);
    this.pending.delete(id);
    const error = new Error("RPC request cancelled");
    error.name = "AbortError";
    request.reject(error);
    this.onOperation({
      requestId: clientRequestId,
      method: request.method,
      phase: "cancelled",
    });
    return true;
  }

  deferredCancellationRequested(clientRequestId: string): boolean {
    return this.deferred.get(clientRequestId)?.cancellationRequested === true;
  }

  settleDeferred(
    clientRequestId: string,
    outcome: "succeeded" | "cancelled" | "failed",
    error?: string,
  ): boolean {
    const deferred = this.deferred.get(clientRequestId);
    if (!deferred) return false;
    this.deferred.delete(clientRequestId);
    this.onOperation({
      requestId: clientRequestId,
      method: deferred.method,
      phase: outcome,
      ...(error ? { error } : {}),
      ...(outcome === "cancelled" ? { errorCode: -32800 } : {}),
    });
    return true;
  }

  acceptChunk(chunk: string): void {
    this.buffer += chunk;
    let newline: number;
    while ((newline = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, newline).trim();
      this.buffer = this.buffer.slice(newline + 1);
      if (!line) continue;
      let response: RpcResponse;
      try {
        response = JSON.parse(line) as RpcResponse;
      } catch {
        continue;
      }
      if (response.id === undefined) {
        if (response.method) {
          this.onNotification(response.method, response.params);
          this.acceptOperationNotification(response.method, response.params);
        }
        continue;
      }
      const request = this.pending.get(response.id);
      if (!request) continue;
      this.pending.delete(response.id);
      if (request.clientRequestId) {
        this.clientRequests.delete(request.clientRequestId);
        if (!response.error && request.deferTerminal) {
          this.deferred.set(request.clientRequestId, {
            method: request.method,
            cancellationRequested: request.cancellationRequested,
          });
        }
      }
      if (response.error) {
        const error = new Error(response.error.message ?? "sidecar RPC failed");
        if (response.error.code === -32800) error.name = "AbortError";
        if (request.clientRequestId) {
          this.onOperation({
            requestId: request.clientRequestId,
            method: request.method,
            phase: response.error.code === -32800 ? "cancelled" : "failed",
            error: error.message,
            ...(typeof response.error.code === "number"
              ? { errorCode: response.error.code }
              : {}),
          });
        }
        request.reject(error);
      } else {
        if (request.clientRequestId && !request.deferTerminal) {
          this.onOperation({
            requestId: request.clientRequestId,
            method: request.method,
            phase: "succeeded",
          });
        }
        request.resolve(response.result);
      }
    }
  }

  rejectAll(reason: string): void {
    const requests = [...this.pending.values()];
    const deferred = [...this.deferred.entries()];
    this.pending.clear();
    this.clientRequests.clear();
    this.deferred.clear();
    requests.forEach(({ clientRequestId, method, reject }) => {
      if (clientRequestId) {
        this.onOperation({
          requestId: clientRequestId,
          method,
          phase: "failed",
          error: reason,
        });
      }
      reject(new Error(reason));
    });
    deferred.forEach(([requestId, request]) => {
      this.onOperation({
        requestId,
        method: request.method,
        phase: "failed",
        error: reason,
      });
    });
  }

  private acceptOperationNotification(method: string, params: unknown): void {
    if (
      method !== "$/progress"
      || !params
      || typeof params !== "object"
      || !("token" in params)
      || typeof params.token !== "string"
    ) {
      return;
    }
    const id = this.clientRequests.get(params.token);
    const request = id === undefined ? undefined : this.pending.get(id);
    if (
      !request
      || request.clientRequestId !== params.token
      || request.cancellationRequested
    ) {
      return;
    }
    this.onOperation({
      requestId: params.token,
      method: request.method,
      phase: "progress",
      ...("stage" in params && typeof params.stage === "string"
        ? { stage: params.stage }
        : {}),
    });
  }

  private requestCancellation(id: number, request: PendingRequest): void {
    if (request.cancellationRequested) return;
    request.cancellationRequested = true;
    this.writeLine(JSON.stringify({
      jsonrpc: "2.0",
      method: "$/cancelRequest",
      params: { id },
    }));
  }
}
