import { describe, expect, it } from "vitest";
import { SidecarRpcClient } from "./sidecar-rpc";

describe("SidecarRpcClient", () => {
  it("sends protocol cancellation and rejects the matching renderer request", async () => {
    const lines: string[] = [];
    const client = new SidecarRpcClient((line) => lines.push(line));
    const request = client.request("mt.query", { index: 4 }, "dock-7");

    expect(client.cancel("dock-7")).toBe(true);
    await expect(request).rejects.toMatchObject({
      name: "AbortError",
      message: "RPC request cancelled",
    });
    expect(lines.map((line) => JSON.parse(line))).toEqual([
      {
        jsonrpc: "2.0",
        id: 1,
        method: "mt.query",
        params: { index: 4 },
      },
      {
        jsonrpc: "2.0",
        method: "$/cancelRequest",
        params: { id: 1 },
      },
    ]);
  });

  it("routes out-of-order NDJSON responses and ignores late cancelled output", async () => {
    const lines: string[] = [];
    const notifications: Array<{ method: string; params: unknown }> = [];
    const client = new SidecarRpcClient(
      (line) => lines.push(line),
      (method, params) => notifications.push({ method, params }),
    );
    const first = client.request("search.run", { query: "one" }, "search-1");
    const second = client.request("dict.query", { word: "two" }, "dict-2");

    client.acceptChunk(
      [
        JSON.stringify({
          jsonrpc: "2.0",
          method: "project.files-changed",
          params: { root: "/project", paths: ["/project/source/new.txt"] },
        }),
        JSON.stringify({ jsonrpc: "2.0", id: 2, result: ["definition"] }),
        "",
      ].join("\n"),
    );
    expect(await second).toEqual(["definition"]);
    expect(notifications).toEqual([{
      method: "project.files-changed",
      params: { root: "/project", paths: ["/project/source/new.txt"] },
    }]);
    expect(client.cancel("search-1")).toBe(true);
    await expect(first).rejects.toMatchObject({ name: "AbortError" });

    client.acceptChunk(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        error: { code: -32800, message: "request cancelled" },
      })}\n`,
    );
    expect(lines).toHaveLength(3);
  });

  it("publishes exact long-operation progress and terminal states", async () => {
    const lines: string[] = [];
    const operations: unknown[] = [];
    const client = new SidecarRpcClient(
      (line) => lines.push(line),
      () => undefined,
      (event) => operations.push(event),
    );
    const compile = client.request(
      "project.compile",
      { progress_token: "operation-compile-1" },
      "operation-compile-1",
    );
    client.acceptChunk([
      JSON.stringify({
        jsonrpc: "2.0",
        method: "$/progress",
        params: { token: "operation-compile-1", stage: "compile:filters" },
      }),
      JSON.stringify({ jsonrpc: "2.0", id: 1, result: { files: 3 } }),
      "",
    ].join("\n"));
    await expect(compile).resolves.toEqual({ files: 3 });

    const team = client.request(
      "team.sync",
      { progress_token: "operation-teamSync-2" },
      "operation-teamSync-2",
    );
    let teamSettled = false;
    void team.finally(() => {
      teamSettled = true;
    }).catch(() => undefined);
    expect(client.cancel("operation-teamSync-2")).toBe(true);
    await Promise.resolve();
    expect(teamSettled).toBe(false);
    client.acceptChunk(`${JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      error: { code: -32800, message: "request cancelled" },
    })}\n`);
    await expect(team).rejects.toMatchObject({
      name: "AbortError",
      message: "request cancelled",
    });

    expect(operations).toEqual([
      {
        requestId: "operation-compile-1",
        method: "project.compile",
        phase: "started",
      },
      {
        requestId: "operation-compile-1",
        method: "project.compile",
        phase: "progress",
        stage: "compile:filters",
      },
      {
        requestId: "operation-compile-1",
        method: "project.compile",
        phase: "succeeded",
      },
      {
        requestId: "operation-teamSync-2",
        method: "team.sync",
        phase: "started",
      },
      {
        requestId: "operation-teamSync-2",
        method: "team.sync",
        phase: "cancelling",
      },
      {
        requestId: "operation-teamSync-2",
        method: "team.sync",
        phase: "cancelled",
        error: "request cancelled",
      },
    ]);
    expect(lines.map((line) => JSON.parse(line)).at(-1)).toEqual({
      jsonrpc: "2.0",
      method: "$/cancelRequest",
      params: { id: 2 },
    });
  });
});
