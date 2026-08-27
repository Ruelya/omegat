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
});
