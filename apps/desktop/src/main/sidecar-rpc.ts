// SPDX-License-Identifier: GPL-3.0-or-later

type PendingRequest = {
  clientRequestId: string | null;
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
};

type RpcResponse = {
  id?: number;
  result?: unknown;
  error?: { code?: number; message?: string };
};

export class SidecarRpcClient {
  private readonly pending = new Map<number, PendingRequest>();
  private readonly clientRequests = new Map<string, number>();
  private nextId = 1;
  private buffer = "";

  constructor(private readonly writeLine: (line: string) => void) {}

  request(
    method: string,
    params: unknown = {},
    clientRequestId: string | null = null,
  ): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { clientRequestId, resolve, reject });
      if (clientRequestId) this.clientRequests.set(clientRequestId, id);
      this.writeLine(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
    });
  }

  /**
   * Cancel a renderer request on the NDJSON wire, not only in the publishing
   * component. The sidecar maps this notification to its cooperative token.
   */
  cancel(clientRequestId: string): boolean {
    const id = this.clientRequests.get(clientRequestId);
    if (id === undefined) return false;
    const request = this.pending.get(id);
    this.clientRequests.delete(clientRequestId);
    this.pending.delete(id);
    this.writeLine(JSON.stringify({
      jsonrpc: "2.0",
      method: "$/cancelRequest",
      params: { id },
    }));
    const error = new Error("RPC request cancelled");
    error.name = "AbortError";
    request?.reject(error);
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
      if (response.id === undefined) continue;
      const request = this.pending.get(response.id);
      if (!request) continue;
      this.pending.delete(response.id);
      if (request.clientRequestId) {
        this.clientRequests.delete(request.clientRequestId);
      }
      if (response.error) {
        request.reject(new Error(response.error.message ?? "sidecar RPC failed"));
      } else {
        request.resolve(response.result);
      }
    }
  }

  rejectAll(reason: string): void {
    const requests = [...this.pending.values()];
    this.pending.clear();
    this.clientRequests.clear();
    requests.forEach(({ reject }) => reject(new Error(reason)));
  }
}
