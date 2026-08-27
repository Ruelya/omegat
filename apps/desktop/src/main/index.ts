import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from "electron";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { detectLocale, setLocale } from "../renderer/i18n";
import { isLongOperationMethod } from "../shared/rpc-operation";
import {
  createApplicationLifecycle,
  registerApplicationLifecycle,
} from "./lifecycle";
import { buildApplicationMenu } from "./menu";
import {
  ProjectFileWatcher,
  type ExternalProjectChange,
} from "./project-file-watcher";
import { SidecarRpcClient } from "./sidecar-rpc";

let sidecar: ChildProcessWithoutNullStreams | null = null;
let rpcClient: SidecarRpcClient | null = null;
let sidecarRecovery: Promise<void> | null = null;
let stoppingSidecar = false;
const isolatedMarkerSidecars = new Set<ChildProcessWithoutNullStreams>();
const appInstance = randomUUID();
let nextId = 1;
const watchedProjectWriteMethods = new Set([
  "project.save",
  "project.compile",
  "project.close",
  "project.update",
  "project.import",
  "team.mapping",
  "team.sync",
  "team.commit",
  "team.resolve",
  "glossary.add",
  "spell.ignore",
  "spell.learn",
  "wiki.import",
]);
const projectFileWatcher = new ProjectFileWatcher((event) => {
  void persistExternalProjectChange(event);
});

if (process.env.OMEGAT_CONFIG_DIR) {
  app.setPath("userData", process.env.OMEGAT_CONFIG_DIR);
}

function sidecarName(): string {
  return process.platform === "win32" ? "omegat-sidecar.exe" : "omegat-sidecar";
}

function sidecarPath(): string {
  const name = sidecarName();
  const extra = join(process.resourcesPath, name);
  const dev = join(app.getAppPath(), "..", "..", "target", "debug", name);
  const rel = join(app.getAppPath(), "..", "..", "target", "release", name);
  if (existsSync(extra)) return extra;
  if (existsSync(rel)) return rel;
  return dev;
}

function manualPath(locale = "en"): string {
  const name = locale.startsWith("zh") ? "zh-CN.md" : "en.md";
  const bundled = join(process.resourcesPath, "manual", name);
  const javaHtml = join(process.resourcesPath, "manual", "java", "index.html");
  const dev = join(app.getAppPath(), "..", "..", "docs", "manual", name);
  if (existsSync(bundled)) return bundled;
  if (existsSync(javaHtml)) return javaHtml;
  return dev;
}

function inspectDroppedPaths(paths: unknown) {
  const safe = Array.isArray(paths)
    ? paths.filter((path): path is string => typeof path === "string" && path.trim().length > 0)
    : [];
  const first = safe[0];
  if (!first) return { kind: "files" as const, paths: [] };
  let candidate = basename(first) === "omegat.project" ? dirname(first) : first;
  try {
    if (
      statSync(candidate).isDirectory()
      && existsSync(join(candidate, "omegat.project"))
    ) {
      return { kind: "project" as const, root: candidate };
    }
  } catch {
    // The renderer will reject inaccessible paths through project.import.
  }
  return { kind: "files" as const, paths: safe };
}

type RefreshBatch = {
  id: string;
  paths: string[];
  fingerprints: Record<string, string | null>;
  sources: Array<"native" | "sidecar">;
};

function publishRefreshBatch(
  root: string,
  generation: number,
  batch: RefreshBatch,
) {
  BrowserWindow.getAllWindows().forEach((window) => {
    window.webContents.send("project:external-change", {
      root,
      generation,
      ...batch,
    });
  });
}

async function pendingRefreshBatches(
  client: SidecarRpcClient,
  root: string,
  generation: number,
): Promise<RefreshBatch[]> {
  const result = await client.request("project.refresh.pending", {
    root,
    generation,
    app_instance: appInstance,
  }) as { batches?: RefreshBatch[] };
  return Array.isArray(result.batches) ? result.batches : [];
}

async function publishPendingRefreshBatches(
  client: SidecarRpcClient,
  root: string,
  generation: number,
) {
  const batches = await pendingRefreshBatches(client, root, generation);
  batches.forEach((batch) => publishRefreshBatch(root, generation, batch));
}

function scheduleSidecarRecovery() {
  const watched = projectFileWatcher.currentProject();
  if (stoppingSidecar || !watched || sidecarRecovery) return;
  sidecarRecovery = (async () => {
    // Let the child exit and pipe close notifications settle before replacing
    // the stateful process.
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 50));
    const client = startSidecar();
    const endRecoveryWrite = projectFileWatcher.beginWriteSource(
      "project.open.recovery",
    );
    try {
      await client.request("project.open", { root: watched.root });
    } finally {
      // Reopening recreates internal project directories and omegat/.lock.
      // Those writes belong to recovery and must not become a new fingerprint
      // batch behind the one this process is about to replay.
      endRecoveryWrite();
    }
    if (
      projectFileWatcher.currentProject()?.root === watched.root
      && projectFileWatcher.currentProject()?.generation === watched.generation
    ) {
      await publishPendingRefreshBatches(
        client,
        watched.root,
        watched.generation,
      );
    }
  })().catch((error) => {
    process.stderr.write(`sidecar recovery failed: ${String(error)}\n`);
  }).finally(() => {
    sidecarRecovery = null;
    if (!stoppingSidecar && !rpcClient) scheduleSidecarRecovery();
  });
}

function startSidecar(): SidecarRpcClient {
  if (rpcClient && sidecar) return rpcClient;
  const bin = sidecarPath();
  const child = spawn(bin, [], { stdio: ["pipe", "pipe", "pipe"] });
  const client = new SidecarRpcClient(
    (line) => {
      child.stdin.write(`${line}\n`);
    },
    (method, params) => {
      if (
        method !== "project.files-changed"
        || !params
        || typeof params !== "object"
      ) {
        return;
      }
      const root = "root" in params ? params.root : null;
      const paths = "paths" in params ? params.paths : null;
      if (
        typeof root === "string"
        && Array.isArray(paths)
        && paths.every((path): path is string => typeof path === "string")
      ) {
        projectFileWatcher.acceptExternalChange({ root, paths });
      }
    },
    (event) => {
      if (!isLongOperationMethod(event.method)) return;
      BrowserWindow.getAllWindows().forEach((window) => {
        window.webContents.send("rpc:operation", event);
      });
    },
  );
  sidecar = child;
  rpcClient = client;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    client.acceptChunk(chunk);
  });
  child.stderr.on("data", (d: Buffer) => {
    process.stderr.write(d);
  });
  child.once("error", (error) => {
    if (sidecar !== child) return;
    client.rejectAll(`sidecar error: ${error.message}`);
  });
  child.once("exit", (code, signal) => {
    if (sidecar !== child) return;
    client.rejectAll(
      `sidecar exited${signal ? ` (${signal})` : ` (${code ?? "unknown"})`}`,
    );
    sidecar = null;
    rpcClient = null;
    scheduleSidecarRecovery();
  });
  return client;
}

function stopSidecar() {
  stoppingSidecar = true;
  rpcClient?.rejectAll("sidecar stopped");
  rpcClient = null;
  const child = sidecar;
  sidecar = null;
  child?.kill();
  for (const child of isolatedMarkerSidecars) child.kill();
  isolatedMarkerSidecars.clear();
}

async function statefulClient(): Promise<SidecarRpcClient> {
  if (sidecarRecovery) await sidecarRecovery;
  return rpcClient ?? startSidecar();
}

async function persistExternalProjectChange(event: ExternalProjectChange) {
  const persist = async () => {
    const client = await statefulClient();
    return client.request("project.refresh.enqueue", {
      ...event,
      app_instance: appInstance,
    }) as Promise<{ batch?: RefreshBatch }>;
  };
  try {
    let result: { batch?: RefreshBatch };
    try {
      result = await persist();
    } catch {
      if (sidecarRecovery) await sidecarRecovery;
      result = await persist();
    }
    if (result.batch) {
      publishRefreshBatch(event.root, event.generation, result.batch);
    }
  } catch (error) {
    process.stderr.write(
      `cannot persist external refresh fingerprint: ${String(error)}\n`,
    );
  }
}

function isolatedMarkerRpc(method: string, params: unknown): Promise<unknown> {
  const child = spawn(sidecarPath(), [], { stdio: ["pipe", "pipe", "pipe"] });
  isolatedMarkerSidecars.add(child);
  const id = nextId++;
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk: string) => {
    stderr += chunk;
  });
  child.once("close", () => isolatedMarkerSidecars.delete(child));
  child.stdin.end(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error(`isolated marker RPC timed out: ${method}`));
    }, 8_000);
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("exit", (code) => {
      clearTimeout(timeout);
      const line = stdout.trim().split(/\r?\n/).at(-1);
      if (!line) {
        reject(new Error(`isolated marker sidecar exited ${code}: ${stderr.trim()}`));
        return;
      }
      try {
        const response = JSON.parse(line) as {
          result?: unknown;
          error?: { message?: string };
        };
        if (response.error) {
          reject(new Error(response.error.message ?? "isolated marker RPC failed"));
        } else {
          resolve(response.result);
        }
      } catch (error) {
        reject(new Error(`invalid isolated marker response: ${String(error)}`));
      }
    });
  });
}

async function rpc(
  method: string,
  params: unknown = {},
  clientRequestId: string | null = null,
): Promise<unknown> {
  // Native callbacks may take the full worker timeout. Keep project and
  // navigation RPCs responsive while each Marker runs in its own sidecar,
  // which in turn retains the cdylib crash/timeout worker boundary.
  if (method === "markers.query") return isolatedMarkerRpc(method, params);
  const client = await statefulClient();
  const endWrite = watchedProjectWriteMethods.has(method)
    ? projectFileWatcher.beginWriteSource(method)
    : () => undefined;
  return client.request(method, params, clientRequestId).finally(endWrite);
}

function createWindow() {
  const win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 960,
    minHeight: 640,
    backgroundColor: "#f4efe6",
    title: "OmegaT",
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  if (process.env.ELECTRON_RENDERER_URL) {
    win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    win.loadFile(join(__dirname, "../renderer/index.html"));
  }
  Menu.setApplicationMenu(buildApplicationMenu(win));
  win.webContents.on("did-finish-load", () => {
    win.webContents.send("menu:ready");
  });
}

function applyMenuLocale(locale: string) {
  setLocale(locale);
  const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
  if (win) Menu.setApplicationMenu(buildApplicationMenu(win));
}

app.whenReady().then(() => {
  stoppingSidecar = false;
  startSidecar();
  ipcMain.handle(
    "rpc",
    (_e, method: string, params: unknown, clientRequestId?: string) =>
      rpc(method, params, clientRequestId ?? null),
  );
  ipcMain.handle("rpc-cancel", (_e, clientRequestId: string) =>
    rpcClient?.cancel(clientRequestId) ?? false
  );
  ipcMain.handle("startup-context", () => ({
    project: process.env.OMEGAT_PROJECT || null,
    configDir: process.env.OMEGAT_CONFIG_DIR || app.getPath("userData"),
    scriptsDir: process.env.OMEGAT_SCRIPTS_DIR || null,
  }));
  ipcMain.handle("pick-dir", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] });
    return r.canceled ? null : r.filePaths[0];
  });
  ipcMain.handle("pick-file", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openFile"] });
    return r.canceled ? null : r.filePaths[0];
  });
  ipcMain.handle("pick-files", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openFile", "multiSelections"] });
    return r.canceled ? null : r.filePaths;
  });
  ipcMain.handle("inspect-drop", (_e, paths: unknown) => inspectDroppedPaths(paths));
  ipcMain.handle("project-watch", async (_e, root: string, generation?: number) => {
    if (typeof root === "string" && root.trim()) {
      const activeGeneration = projectFileWatcher.watch(
        root,
        typeof generation === "number" ? generation : undefined,
      );
      const client = await statefulClient();
      await publishPendingRefreshBatches(client, root, activeGeneration);
    }
  });
  ipcMain.handle(
    "project-refresh-complete",
    async (
      _e,
      root: string,
      generation: number,
      batchId: string,
      outcome: "succeeded" | "cancelled" | "coalesced",
    ) => {
      const active = projectFileWatcher.currentProject();
      if (
        !active
        || active.root !== root
        || active.generation !== generation
      ) {
        return { remaining: [] };
      }
      return rpc("project.refresh.complete", {
        root,
        generation,
        batch_id: batchId,
        outcome,
        app_instance: appInstance,
      });
    },
  );
  ipcMain.handle("project-unwatch", async () => {
    const active = projectFileWatcher.currentProject();
    if (active) {
      try {
        await rpc("project.refresh.discard", {
          root: active.root,
          generation: active.generation,
          app_instance: appInstance,
        });
      } catch {
        // A terminated process leaves the queue for crash recovery.
      }
    }
    projectFileWatcher.close();
  });
  ipcMain.handle("save-text", async (_e, name: string, text: string) => {
    const r = await dialog.showSaveDialog({ defaultPath: name });
    if (r.canceled || !r.filePath) return null;
    writeFileSync(r.filePath, text, "utf8");
    return r.filePath;
  });
  registerApplicationLifecycle(
    ipcMain,
    createApplicationLifecycle(app, stopSidecar),
  );
  ipcMain.handle("open-path", async (_e, path: string) => {
    if (path) await shell.openPath(path);
  });
  ipcMain.handle("open-external", async (_e, url: string) => {
    if (url) await shell.openExternal(url);
  });
  ipcMain.handle("open-manual", async () => {
    const local = manualPath();
    if (existsSync(local)) await shell.openPath(local);
    else await shell.openExternal("https://omegat.org/manual");
  });
  ipcMain.handle("menu-locale", (_e, locale: string) => {
    applyMenuLocale(typeof locale === "string" ? locale : "en");
  });
  setLocale(detectLocale(process.env.OMEGAT_LOCALE || app.getLocale()));
  createWindow();
});

app.on("window-all-closed", () => {
  projectFileWatcher.close();
  stopSidecar();
  if (process.platform !== "darwin") app.quit();
});
