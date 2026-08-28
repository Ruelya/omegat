import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from "electron";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  appendFileSync,
  closeSync,
  existsSync,
  fsyncSync,
  openSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";
import { detectLocale, setLocale } from "../renderer/i18n";
import { isLongOperationMethod } from "../shared/rpc-operation";
import type {
  TransactionEnvelope,
  TransactionOutcome,
} from "../shared/transaction-envelope";
import {
  createApplicationLifecycle,
  registerApplicationLifecycle,
} from "./lifecycle";
import { buildApplicationMenu } from "./menu";
import {
  ProjectFileWatcher,
  type ExternalProjectChange,
} from "./project-file-watcher";
import {
  scopeProductTransaction,
  transactionEnvelopesForRenderer,
} from "./product-transaction-scope";
import { SidecarRpcClient } from "./sidecar-rpc";

let sidecar: ChildProcessWithoutNullStreams | null = null;
let rpcClient: SidecarRpcClient | null = null;
let sidecarRecovery: Promise<void> | null = null;
let detachedRecovery: Promise<void> | null = null;
let stoppingSidecar = false;
const isolatedMarkerSidecars = new Set<ChildProcessWithoutNullStreams>();
const appInstance = randomUUID();
let nextId = 1;
type DetachedTransactionScope = {
  root: string;
  generation: number;
  sidecarProjectOpen: boolean;
};
let detachedTransactionScope: DetachedTransactionScope | null = null;
const watchedProjectWriteMethods = new Set([
  "entry.set",
  "project.save",
  "project.reload",
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
  "align.run",
  "align.write",
]);
const projectFileWatcher = new ProjectFileWatcher((event) => {
  void persistExternalProjectChange(event);
});

if (process.env.OMEGAT_CONFIG_DIR?.trim()) {
  // OmegaT configuration is intentionally shared across application
  // instances. Chromium profiles are not: sharing userData makes Chromium's
  // ProcessSingleton abort the second Electron process before OmegaT can
  // isolate either project's durable transaction queue.
  app.setPath(
    "userData",
    join(process.env.OMEGAT_CONFIG_DIR, "electron-instances", appInstance),
  );
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

function normalizedProjectRoot(root: string): string {
  try {
    return realpathSync(root);
  } catch {
    return root;
  }
}

function publishTransactionEnvelope(
  root: string,
  generation: number,
  envelope: TransactionEnvelope,
) {
  if (
    envelope.version !== 1
    || envelope.generation !== generation
    || normalizedProjectRoot(envelope.project_root) !== normalizedProjectRoot(root)
    || !["pending", "sidecar_committed"].includes(envelope.status)
    || typeof envelope.payload?.operation !== "string"
  ) return;
  const trace = process.env.OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE;
  if (trace) {
    appendFileSync(trace, `${JSON.stringify({
      batch_id: envelope.batch_id,
      operation: envelope.payload.operation,
      project_root: envelope.project_root,
      generation: envelope.generation,
      status: envelope.status,
    })}\n`);
  }
  BrowserWindow.getAllWindows().forEach((window) => {
    window.webContents.send("transaction:envelope", envelope);
  });
}

function durableTestMarker(path: string, value: unknown) {
  writeFileSync(path, `${JSON.stringify(value)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  const marker = openSync(path, "r");
  try {
    fsyncSync(marker);
  } finally {
    closeSync(marker);
  }
  const parent = openSync(dirname(path), "r");
  try {
    fsyncSync(parent);
  } finally {
    closeSync(parent);
  }
}

function killSidecarAfterSelectedTransactionHead(
  envelopes: TransactionEnvelope[],
): boolean {
  const operation =
    process.env.OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_FOR;
  const markerPath =
    process.env.OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_MARKER;
  const envelope = envelopes[0];
  const child = sidecar;
  if (
    !operation
    || !markerPath
    || !envelope
    || envelope.payload.operation !== operation
    || existsSync(markerPath)
    || !child?.pid
  ) {
    return false;
  }
  // The pending query has selected the durable global FIFO head, but the
  // renderer has not seen (and therefore cannot acknowledge) it yet. Kill only
  // the stateful sidecar; its replacement must select this exact head again.
  durableTestMarker(markerPath, {
    batch_id: envelope.batch_id,
    operation,
    sidecar_pid: child.pid,
    signal: "SIGKILL",
  });
  child.kill("SIGKILL");
  return true;
}

async function holdAfterClaimingTransactionHead(
  envelopes: TransactionEnvelope[],
  cancellationRequested: () => boolean = () => false,
): Promise<boolean> {
  const operation =
    process.env.OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR;
  const markerPath =
    process.env.OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER;
  const releasePath =
    process.env.OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE;
  const envelope = envelopes[0];
  if (
    !operation
    || !markerPath
    || !releasePath
    || !envelope
    || envelope.payload.operation !== operation
    || existsSync(markerPath)
  ) {
    return cancellationRequested();
  }
  durableTestMarker(markerPath, {
    batch_id: envelope.batch_id,
    operation,
    app_instance: appInstance,
    owner_process_id: process.pid,
    generation: envelope.generation,
  });
  while (
    !stoppingSidecar
    && !existsSync(releasePath)
    && !cancellationRequested()
  ) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
  }
  return cancellationRequested();
}

async function holdBeforeTransactionDispatch(): Promise<void> {
  const markerPath =
    process.env.OMEGAT_TEST_HOLD_BEFORE_TRANSACTION_DISPATCH_MARKER;
  const releasePath =
    process.env.OMEGAT_TEST_HOLD_BEFORE_TRANSACTION_DISPATCH_RELEASE;
  if (!markerPath || !releasePath || existsSync(markerPath)) return;
  durableTestMarker(markerPath, {
    app_instance: appInstance,
    owner_process_id: process.pid,
  });
  while (!stoppingSidecar && !existsSync(releasePath)) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
  }
}

function traceTransactionOwnerRetry(value: unknown) {
  const trace = process.env.OMEGAT_TEST_TRANSACTION_OWNER_RETRY_TRACE;
  if (trace) appendFileSync(trace, `${JSON.stringify(value)}\n`);
}

async function cancelCommittedResolveReceipt(
  client: SidecarRpcClient,
  root: string,
  generation: number,
  receipt: { batchId: string; operation: string },
): Promise<never> {
  if (receipt.operation !== "resolve-conflict") {
    throw new Error("only an undelivered team.resolve receipt can be cancelled");
  }
  try {
    await client.request("transaction.receipt.ack", {
      root,
      generation,
      batch_id: receipt.batchId,
      operation: receipt.operation,
      outcome: "cancelled",
      app_instance: appInstance,
      owner_process_id: process.pid,
    });
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") throw error;
    throw error;
  }
  throw new Error("sidecar accepted cancellation without protocol -32800");
}

async function publishPendingTransactionEnvelopes(
  client: SidecarRpcClient,
  root: string,
  generation: number,
  clientRequestId?: string | null,
  expectedReceipt?: { batchId: string; operation: string } | null,
  callerManagesExpectedReceipt = false,
) {
  let result: {
    envelopes?: TransactionEnvelope[];
    owner_retry?: {
      previous_owner_process_id?: number;
      previous_owner_process_ids?: number[];
    } | null;
  };
  try {
    result = await client.request(
      "transaction.receipt.pending",
      {
        root,
        generation,
        app_instance: appInstance,
        owner_process_id: process.pid,
        // Linux is the platform where dispatcher ownership has a real PID
        // liveness contract. A losing packaged process stays alive across two
        // owner deaths, then rejects if it also loses the third election.
        ...(process.platform === "linux"
          ? {
              owner_retry_timeout_ms: 300_000,
              owner_retry_attempts: 2,
            }
          : {}),
      },
      null,
      false,
      clientRequestId ?? null,
    ) as typeof result;
  } catch (error) {
    const message = String(error);
    const previousOwner = message.match(
      /replacement retry after owner pid (\d+) exited/,
    );
    if (previousOwner) {
      traceTransactionOwnerRetry({
        result: "rejected",
        replacement_process_id: process.pid,
        previous_owner_process_id: Number(previousOwner[1]),
        error: message,
      });
    }
    if (
      error instanceof Error
      && error.name === "AbortError"
      && clientRequestId
      && client.deferredCancellationRequested(clientRequestId)
      && expectedReceipt
    ) {
      await cancelCommittedResolveReceipt(
        client,
        root,
        generation,
        expectedReceipt,
      );
    }
    throw error;
  }
  if (typeof result.owner_retry?.previous_owner_process_id === "number") {
    traceTransactionOwnerRetry({
      result: "claimed",
      replacement_process_id: process.pid,
      previous_owner_process_id: result.owner_retry.previous_owner_process_id,
      previous_owner_process_ids:
        result.owner_retry.previous_owner_process_ids
        ?? [result.owner_retry.previous_owner_process_id],
    });
  }
  const envelopes = Array.isArray(result.envelopes) ? result.envelopes : [];
  const head = envelopes[0];
  const detached = detachedTransactionScope;
  if (
    head?.status === "pending"
    && head.payload.operation === "project.external-refresh"
    && detached
    && normalizedProjectRoot(detached.root) === normalizedProjectRoot(root)
    && detached.generation === generation
    && !detached.sidecarProjectOpen
  ) {
    const endRecoveryWrite = projectFileWatcher.beginWriteSource(
      "project.open.detached-recovery",
    );
    try {
      await client.request("project.open", { root });
      detached.sidecarProjectOpen = true;
    } finally {
      endRecoveryWrite();
    }
  }
  const cancelled = await holdAfterClaimingTransactionHead(
    envelopes,
    () =>
      Boolean(
        clientRequestId
        && client.deferredCancellationRequested(clientRequestId)
      ),
  );
  if (cancelled) {
    if (!expectedReceipt) {
      throw new Error("cancelled team.resolve has no scoped receipt");
    }
    await cancelCommittedResolveReceipt(
      client,
      root,
      generation,
      expectedReceipt,
    );
  }
  if (killSidecarAfterSelectedTransactionHead(envelopes)) return -1;
  const rendererEnvelopes = transactionEnvelopesForRenderer(
    envelopes,
    expectedReceipt,
    callerManagesExpectedReceipt,
  );
  rendererEnvelopes.forEach((envelope) =>
    publishTransactionEnvelope(root, generation, envelope)
  );
  return rendererEnvelopes.length;
}

async function advanceDetachedTransactionRecovery(
  client: SidecarRpcClient,
) {
  while (!stoppingSidecar) {
    let scope = detachedTransactionScope;
    const watched = projectFileWatcher.currentProject();
    if (!scope) {
      if (watched) return;
      const discovered = await client.request(
        "transaction.receipt.discover",
        {},
      ) as {
        projects?: Array<{
          project_root?: string;
          generation?: number;
        }>;
      };
      const candidate = discovered.projects?.find((project) =>
        typeof project.project_root === "string"
        && project.project_root.length > 0
      );
      if (!candidate?.project_root) return;
      const previousGeneration = typeof candidate.generation === "number"
        ? candidate.generation
        : 0;
      scope = {
        root: candidate.project_root,
        generation: Math.max(1, previousGeneration + 1),
        sidecarProjectOpen: false,
      };
      detachedTransactionScope = scope;
    } else if (
      watched
      && normalizedProjectRoot(watched.root) !== normalizedProjectRoot(scope.root)
    ) {
      return;
    }

    const published = await publishPendingTransactionEnvelopes(
      client,
      scope.root,
      scope.generation,
    );
    if (published !== 0) return;
    if (scope.sidecarProjectOpen) {
      await client.request("project.recovery.detach", { root: scope.root });
    }
    if (detachedTransactionScope === scope) detachedTransactionScope = null;
    if (projectFileWatcher.currentProject()) return;
    // A config directory may contain more than one detached close receipt.
    // Finish only the selected root, then discover the next exact root.
  }
}

function scheduleDetachedTransactionRecovery(
  replacementClient?: SidecarRpcClient,
) {
  if (stoppingSidecar || detachedRecovery) return;
  detachedRecovery = (async () => {
    const client = replacementClient ?? await statefulClient();
    await advanceDetachedTransactionRecovery(client);
  })().catch((error) => {
    process.stderr.write(`detached transaction recovery failed: ${String(error)}\n`);
  }).finally(() => {
    detachedRecovery = null;
    if (!stoppingSidecar && !rpcClient && !sidecarRecovery) {
      setTimeout(() => scheduleDetachedTransactionRecovery(), 50);
    }
  });
}

function scheduleSidecarRecovery() {
  const watched = projectFileWatcher.currentProject();
  const detached = detachedTransactionScope;
  if (stoppingSidecar || (!watched && !detached) || sidecarRecovery) return;
  sidecarRecovery = (async () => {
    // Let the child exit and pipe close notifications settle before replacing
    // the stateful process.
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 50));
    const client = startSidecar();
    if (detached) {
      detached.sidecarProjectOpen = false;
      await advanceDetachedTransactionRecovery(client);
    } else if (watched) {
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
        await publishPendingTransactionEnvelopes(
          client,
          watched.root,
          watched.generation,
        );
      }
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
    }) as Promise<{ batch?: TransactionEnvelope }>;
  };
  try {
    let result: { batch?: TransactionEnvelope };
    try {
      result = await persist();
    } catch {
      if (sidecarRecovery) await sidecarRecovery;
      result = await persist();
    }
    if (result.batch) {
      await publishPendingTransactionEnvelopes(
        await statefulClient(),
        event.root,
        event.generation,
      );
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
  callerManagesTransactionReceipt = false,
): Promise<unknown> {
  // Native callbacks may take the full worker timeout. Keep project and
  // navigation RPCs responsive while each Marker runs in its own sidecar,
  // which in turn retains the cdylib crash/timeout worker boundary.
  if (method === "markers.query") return isolatedMarkerRpc(method, params);
  const client = await statefulClient();
  let requestParams = method === "project.external-refresh"
      && params !== null
      && typeof params === "object"
      && "transaction_batch_id" in params
    ? { ...params, app_instance: appInstance }
    : params;
  requestParams = scopeProductTransaction(
    method,
    requestParams,
    projectFileWatcher.currentProject(),
    randomUUID,
  );
  const endWrite = watchedProjectWriteMethods.has(method)
    ? projectFileWatcher.beginWriteSource(method)
    : () => undefined;
  const deferredResolve = method === "team.resolve" && clientRequestId !== null;
  const result = await client.request(
    method,
    requestParams,
    clientRequestId,
    deferredResolve,
  )
    .finally(endWrite);
  try {
  const receipt = result !== null
      && typeof result === "object"
      && "receipt" in result
    ? result.receipt
    : null;
  if (
    receipt !== null
    && typeof receipt === "object"
    && "project_root" in receipt
    && "generation" in receipt
    && typeof receipt.project_root === "string"
    && typeof receipt.generation === "number"
  ) {
    if (
      "payload" in receipt
      && receipt.payload !== null
      && typeof receipt.payload === "object"
      && "operation" in receipt.payload
      && receipt.payload.operation === "project.close"
    ) {
      detachedTransactionScope = {
        root: receipt.project_root,
        generation: receipt.generation,
        sidecarProjectOpen: false,
      };
    }
    await publishPendingTransactionEnvelopes(
      client,
      receipt.project_root,
      receipt.generation,
      deferredResolve ? clientRequestId : null,
      "batch_id" in receipt
          && typeof receipt.batch_id === "string"
          && "payload" in receipt
          && receipt.payload !== null
          && typeof receipt.payload === "object"
          && "operation" in receipt.payload
          && typeof receipt.payload.operation === "string"
        ? {
            batchId: receipt.batch_id,
            operation: receipt.payload.operation,
          }
        : null,
      callerManagesTransactionReceipt,
    );
  }
  const scopedExternalRefresh = method === "project.external-refresh"
    && requestParams !== null
    && typeof requestParams === "object"
    && "transaction_batch_id" in requestParams;
  if (scopedExternalRefresh) {
    const trace = process.env.OMEGAT_TEST_EXTERNAL_REFRESH_TRACE;
    if (trace) appendFileSync(trace, `${Date.now()}\n`);
    if (process.env.OMEGAT_TEST_CRASH_AFTER_EXTERNAL_REFRESH_COMMIT === "1") {
      // Packaged fault injection: the sidecar response and durable commit
      // checkpoint exist, but IPC cannot acknowledge the renderer.
      process.kill(process.pid, "SIGKILL");
    }
  }
  if (deferredResolve && clientRequestId) {
    client.settleDeferred(clientRequestId, "succeeded");
  }
  return result;
  } catch (error) {
    if (deferredResolve && clientRequestId) {
      client.settleDeferred(
        clientRequestId,
        error instanceof Error && error.name === "AbortError"
          ? "cancelled"
          : "failed",
        error instanceof Error ? error.message : String(error),
      );
    }
    throw error;
  }
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
    (
      _e,
      method: string,
      params: unknown,
      clientRequestId?: string,
      callerManagesTransactionReceipt?: boolean,
    ) =>
      rpc(
        method,
        params,
        clientRequestId ?? null,
        callerManagesTransactionReceipt === true,
      ),
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
      await holdBeforeTransactionDispatch();
      const client = await statefulClient();
      await publishPendingTransactionEnvelopes(client, root, activeGeneration);
    }
  });
  ipcMain.handle(
    "transaction-receipt-ack",
    async (
      _e,
      envelope: TransactionEnvelope,
      outcome: TransactionOutcome = "succeeded",
    ) => {
      const active = projectFileWatcher.currentProject();
      const activeMatches = Boolean(
        active
        && normalizedProjectRoot(active.root)
          === normalizedProjectRoot(envelope.project_root)
        && active.generation === envelope.generation
      );
      const detachedMatches = Boolean(
        detachedTransactionScope
        && normalizedProjectRoot(detachedTransactionScope.root)
          === normalizedProjectRoot(envelope.project_root)
        && detachedTransactionScope.generation === envelope.generation
      );
      if (
        !activeMatches
        && !detachedMatches
      ) {
        throw new Error(
          "transaction receipt is not scoped to the watched or detached project",
        );
      }
      if (
        process.env.OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR
          === envelope.payload.operation
      ) {
        const trace = process.env.OMEGAT_TEST_TRANSACTION_ACK_TRACE;
        if (trace) {
          appendFileSync(trace, `${JSON.stringify({
            batch_id: envelope.batch_id,
            operation: envelope.payload.operation,
            outcome,
            result: "dropped",
          })}\n`);
        }
        throw new Error(
          `injected lost transaction acknowledgement for ${envelope.batch_id}`,
        );
      }
      const result = await rpc("transaction.receipt.ack", {
        root: envelope.project_root,
        generation: envelope.generation,
        batch_id: envelope.batch_id,
        operation: envelope.payload.operation,
        outcome,
        app_instance: appInstance,
        owner_process_id: process.pid,
      });
      const trace = process.env.OMEGAT_TEST_TRANSACTION_ACK_TRACE;
      if (trace) {
        appendFileSync(trace, `${JSON.stringify({
          batch_id: envelope.batch_id,
          operation: envelope.payload.operation,
          outcome,
          result: "acknowledged",
        })}\n`);
      }
      if (detachedMatches) {
        setTimeout(() => scheduleDetachedTransactionRecovery(), 0);
      } else if (activeMatches) {
        setTimeout(() => {
          void statefulClient()
            .then((client) =>
              publishPendingTransactionEnvelopes(
                client,
                envelope.project_root,
                envelope.generation,
              )
            )
            .catch((error) => {
              process.stderr.write(
                `cannot publish transaction after receipt ack: ${String(error)}\n`,
              );
            });
        }, 0);
      }
      return result;
    },
  );
  ipcMain.handle("project-unwatch", async () => {
    const active = projectFileWatcher.currentProject();
    const preserveDetached = Boolean(
      active
      && detachedTransactionScope
      && normalizedProjectRoot(active.root)
        === normalizedProjectRoot(detachedTransactionScope.root)
      && active.generation === detachedTransactionScope.generation
    );
    // Closing the native watcher is the renderer-visible project boundary.
    // Receipt discovery below must work without retaining this as an implicit
    // active project.
    projectFileWatcher.close();
    if (active && !preserveDetached) {
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
    if (preserveDetached || (!active && !process.env.OMEGAT_PROJECT)) {
      scheduleDetachedTransactionRecovery();
    }
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
