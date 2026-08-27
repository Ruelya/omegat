// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const WAIT_MS = 60_000;
const SOURCE_FILES = 2_400;
const desktopDir = resolve(import.meta.dirname, "..");
const executable =
  process.env.OMEGAT_PACKAGED_EXECUTABLE
  ?? join(desktopDir, "release", "linux-unpacked", "omegat-desktop");
const sidecar =
  process.env.OMEGAT_SIDECAR
  ?? resolve(desktopDir, "..", "..", "target", "release", "omegat-sidecar");
const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

async function waitFor(label, check, timeoutMs = WAIT_MS) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await check();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await sleep(50);
  }
  throw new Error(
    `Timed out waiting for ${label}${lastError ? `: ${lastError.message}` : ""}`,
  );
}

async function unusedPort() {
  const server = createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address();
  assert(address && typeof address === "object");
  await new Promise((resolveClose, reject) =>
    server.close((error) => error ? reject(error) : resolveClose())
  );
  return address.port;
}

async function rpcOnce(configDir, method, params) {
  const child = spawn(sidecar, [], {
    env: { ...process.env, OMEGAT_CONFIG_DIR: configDir },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  child.stdin.end(`${JSON.stringify({ jsonrpc: "2.0", id: 1, method, params })}\n`);
  const code = await new Promise((resolveExit, reject) => {
    child.once("error", reject);
    child.once("exit", resolveExit);
  });
  assert.equal(code, 0, `sidecar setup failed: ${stderr}`);
  const response = stdout
    .trim()
    .split(/\r?\n/)
    .map((line) => JSON.parse(line))
    .find((message) => message.id === 1);
  assert(response, `sidecar setup returned no response: ${stdout}`);
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result;
}

async function startXvfb() {
  const child = spawn(
    "Xvfb",
    ["-displayfd", "3", "-screen", "0", "1440x900x24", "-nolisten", "tcp"],
    { stdio: ["ignore", "ignore", "pipe", "pipe"] },
  );
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const display = await new Promise((resolveDisplay, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Xvfb did not report a display: ${stderr}`)),
      5_000,
    );
    timeout.unref();
    let output = "";
    child.stdio[3].on("data", (chunk) => {
      output += chunk.toString();
      const newline = output.indexOf("\n");
      if (newline >= 0) {
        clearTimeout(timeout);
        resolveDisplay(`:${output.slice(0, newline).trim()}`);
      }
    });
    child.once("error", reject);
    child.once("exit", (code) => {
      reject(new Error(`Xvfb exited early with ${code}: ${stderr}`));
    });
  });
  return { child, display };
}

async function pageTarget(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(1_000),
  });
  if (!response.ok) throw new Error(`DevTools endpoint returned ${response.status}`);
  const targets = await response.json();
  return targets.find((target) => target.type === "page" && target.webSocketDebuggerUrl);
}

class DevToolsClient {
  constructor(url) {
    this.socket = new WebSocket(url);
    this.nextId = 1;
    this.pending = new Map();
  }

  async connect() {
    await new Promise((resolveOpen, reject) => {
      this.socket.addEventListener("open", resolveOpen, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id == null) return;
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      if (message.error) pending.reject(new Error(message.error.message));
      else pending.resolve(message.result);
    });
  }

  command(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolveCommand, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`DevTools command timed out: ${method}`));
      }, 10_000);
      timeout.unref();
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolveCommand(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          reject(error);
        },
      });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression, awaitPromise = false) {
    const response = await this.command("Runtime.evaluate", {
      expression,
      awaitPromise,
      returnByValue: true,
    });
    if (response.exceptionDetails) {
      throw new Error(
        response.exceptionDetails.exception?.description ?? "renderer evaluation failed",
      );
    }
    return response.result?.value;
  }

  close() {
    this.socket.close();
  }
}

async function snapshot(root) {
  const files = [];
  async function walk(dir, prefix = "") {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      const path = join(dir, entry.name);
      if (entry.isDirectory()) await walk(path, relative);
      else if (entry.isFile()) {
        files.push([relative, (await readFile(path)).toString("base64")]);
      }
    }
  }
  await walk(root);
  return files;
}

async function compileResidue(root) {
  const residue = [];
  async function walk(dir, prefix = "") {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.name.includes(".omegat-compile-")) residue.push(relative);
      if (entry.isDirectory()) await walk(join(dir, entry.name), relative);
    }
  }
  await walk(root);
  return residue.sort();
}

async function writeSources(sourceDir) {
  const payload = "x".repeat(4_096);
  for (let start = 0; start < SOURCE_FILES; start += 100) {
    await Promise.all(
      Array.from(
        { length: Math.min(100, SOURCE_FILES - start) },
        (_, offset) => {
          const index = start + offset;
          return writeFile(
            join(sourceDir, `${String(index).padStart(4, "0")}.txt`),
            `Source ${index}: ${payload}`,
            "utf8",
          );
        },
      ),
    );
  }
}

async function terminate(child) {
  if (!child || child.exitCode != null || child.signalCode != null) return;
  const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
  child.kill("SIGTERM");
  await Promise.race([exited, sleep(2_000)]);
}

async function xdotool(display, args) {
  return new Promise((resolveCommand, reject) => {
    const child = spawn("xdotool", args, {
      env: { ...process.env, DISPLAY: display },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) resolveCommand(stdout.trim());
      else reject(new Error(`xdotool ${args[0]} failed (${code}): ${stderr}`));
    });
  });
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises long-operation cancellation in a real Linux package");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-long-cancel-e2e-"));
const configDir = join(workDir, "config");
const project = join(workDir, "project");
const sourceDir = join(project, "source");
const targetDir = join(project, "target");
await mkdir(configDir, { recursive: true });
await rpcOnce(configDir, "project.create", {
  root: project,
  source_lang: "en",
  target_lang: "fr",
  sentence_seg: false,
});
await writeSources(sourceDir);
await mkdir(join(targetDir, "nested"), { recursive: true });
await Promise.all([
  writeFile(join(targetDir, "0000.txt"), "PREEXISTING TARGET", "utf8"),
  writeFile(join(targetDir, "nested", "unrelated.keep"), "MUST REMAIN", "utf8"),
]);
const targetBefore = await snapshot(targetDir);

const port = await unusedPort();
const xvfb = await startXvfb();
let application;
let client;
let stderr = "";
try {
  application = spawn(
    executable,
    [`--remote-debugging-port=${port}`, "--disable-gpu", "--no-sandbox"],
    {
      env: {
        ...process.env,
        DISPLAY: xvfb.display,
        OMEGAT_CONFIG_DIR: configDir,
        OMEGAT_PROJECT: project,
      },
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  application.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const target = await waitFor("packaged renderer", () => pageTarget(port));
  client = new DevToolsClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.command("Runtime.enable");

  await waitFor("large project workspace", async () => {
    const state = await client.evaluate(`(() => {
      document.querySelectorAll(".modal-bg").forEach((modal) => modal.click());
      return {
        project: document.querySelector(".app")?.dataset.projectId ?? null,
        source: document.querySelector(".editor-segment.is-active .src")?.textContent ?? null,
        compileReady: !document.querySelector('[data-operation-action="compile"]')?.disabled,
      };
    })()`);
    if (
      state.project === project
      && state.source?.startsWith("Source ")
      && state.compileReady
    ) {
      return state;
    }
    throw new Error(JSON.stringify(state));
  });

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
    window.__omegatStopOperationTrace = window.omegat.onRpcOperation((event) => {
      window.__omegatRpcOperationTrace.push({
        requestId: event.requestId,
        method: event.method,
        phase: event.phase,
        stage: event.stage ?? "",
        error: event.error ?? "",
      });
    });
    const app = document.querySelector(".app");
    const record = () => {
      const value = [
        app?.dataset.operation ?? "",
        app?.dataset.operationPhase ?? "",
        app?.dataset.operationStage ?? "",
        document.querySelector("[data-operation-status]")?.textContent ?? "",
      ].join("|");
      const trace = window.__omegatDomOperationTrace;
      if (trace.at(-1) !== value) trace.push(value);
    };
    window.__omegatOperationObserver = new MutationObserver(record);
    window.__omegatOperationObserver.observe(app, {
      attributes: true,
      subtree: true,
      childList: true,
      characterData: true,
    });
    record();
  })()`);

  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector('[data-operation-action="compile"]');
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible compile action was unavailable",
  );

  const visibleProgress = await waitFor("visible compile progress checkpoint", async () => {
    const state = await client.evaluate(`(() => {
      const app = document.querySelector(".app");
      return {
        requestId: app?.dataset.operationRequestId ?? "",
        operation: app?.dataset.operation ?? "",
        phase: app?.dataset.operationPhase ?? "",
        stage: app?.dataset.operationStage ?? "",
        status: document.querySelector("[data-operation-status]")?.textContent ?? "",
        cancelVisible: Boolean(document.querySelector('[data-operation-action="cancel"]')),
      };
    })()`);
    if (
      state.operation === "compile"
      && state.phase === "progress"
      && state.stage === "project.compile.targets"
      && state.status === "compile: project.compile.targets"
      && state.cancelVisible
    ) {
      return state;
    }
    throw new Error(JSON.stringify(state));
  });

  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector('[data-operation-action="cancel"]');
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible cancel action was unavailable",
  );

  const cancelled = await waitFor("protocol-confirmed visible cancellation", async () => {
    const state = await client.evaluate(`(() => {
      const app = document.querySelector(".app");
      return {
        operation: app?.dataset.operation ?? "",
        phase: app?.dataset.operationPhase ?? "",
        stage: app?.dataset.operationStage ?? "",
        operationStatus: document.querySelector("[data-operation-status]")?.textContent ?? "",
        footer: [...document.querySelectorAll("footer.status span")]
          .map((node) => node.textContent ?? ""),
        cancelVisible: Boolean(document.querySelector('[data-operation-action="cancel"]')),
      };
    })()`);
    if (
      state.operation === "compile"
      && state.phase === "cancelled"
      && state.stage === "project.compile.targets"
      && state.operationStatus
        === "compile: cancelled (project.compile.targets)"
      && !state.cancelVisible
    ) {
      return state;
    }
    throw new Error(JSON.stringify(state));
  });

  const postCancel = await client.evaluate(`(async () => ({
    version: await window.omegat.rpc("sys.version", {}),
    entries: (await window.omegat.rpc("entry.list", {})).length,
    rpcTrace: window.__omegatRpcOperationTrace,
    domTrace: window.__omegatDomOperationTrace,
  }))()`, true);
  const requestTrace = postCancel.rpcTrace
    .filter((event) => event.requestId === visibleProgress.requestId)
    .map((event) => `${event.phase}:${event.stage}`);
  assert.deepEqual(requestTrace, [
    "started:",
    "progress:project.compile.targets",
    "cancelling:",
    "cancelled:",
  ]);
  assert(
    postCancel.domTrace.includes(
      "compile|cancelling|project.compile.targets|compile: cancelling (project.compile.targets)",
    ),
    `renderer never visibly entered cancelling: ${JSON.stringify(postCancel.domTrace)}`,
  );
  assert.equal(postCancel.version.rewrite, true);
  assert.equal(postCancel.entries, SOURCE_FILES);
  assert.deepEqual(await snapshot(targetDir), targetBefore);
  assert.deepEqual(await compileResidue(project), []);

  const reloadBefore = await client.evaluate(`(() => {
    const active = document.querySelector(".editor-segment.is-active");
    return {
      entry: Number(active?.getAttribute("data-entry") ?? -1),
      key: active?.getAttribute("data-entry-key") ?? "",
      source: active?.querySelector(".src")?.textContent ?? "",
      translation: active?.querySelector(".editor-surface")?.textContent ?? "",
    };
  })()`);
  assert.equal(reloadBefore.entry, 1);
  assert(reloadBefore.key, "active EntryKey was unavailable before reload");
  assert(reloadBefore.source.startsWith("Source 0: "));

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
    window.__omegatReloadProgress = null;
    const app = document.querySelector(".app");
    window.__omegatReloadCancelObserver = new MutationObserver(() => {
      if (
        app?.dataset.operation === "reload"
        && app?.dataset.operationPhase === "progress"
        && app?.dataset.operationStage === "project.reload.sources"
      ) {
        const button = document.querySelector('[data-operation-action="cancel"]');
        if (!button || window.__omegatReloadProgress) return;
        window.__omegatReloadProgress = {
          requestId: app.dataset.operationRequestId ?? "",
          operation: app.dataset.operation,
          phase: app.dataset.operationPhase,
          stage: app.dataset.operationStage,
          status: document.querySelector("[data-operation-status]")?.textContent ?? "",
          cancelVisible: true,
        };
        button.click();
      }
    });
    window.__omegatReloadCancelObserver.observe(app, {
      attributes: true,
      subtree: true,
      childList: true,
      characterData: true,
    });
  })()`);
  const windowId = await waitFor("OmegaT X11 window", async () => {
    const ids = await xdotool(xvfb.display, [
      "search",
      "--sync",
      "--onlyvisible",
      "--name",
      "OmegaT",
    ]);
    return ids.split(/\s+/).filter(Boolean).at(-1);
  });
  await xdotool(xvfb.display, ["windowfocus", "--sync", String(windowId)]);
  await xdotool(xvfb.display, ["key", "F5"]);

  const reloadCancelled = await waitFor(
    "protocol-confirmed visible reload cancellation",
    async () => {
      const state = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        const active = document.querySelector(".editor-segment.is-active");
        return {
          operation: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
          stage: app?.dataset.operationStage ?? "",
          operationStatus: document.querySelector("[data-operation-status]")?.textContent ?? "",
          editorStatus: [...document.querySelectorAll("footer.status span")]
            .map((node) => node.textContent ?? ""),
          cancelVisible: Boolean(document.querySelector('[data-operation-action="cancel"]')),
          entry: Number(active?.getAttribute("data-entry") ?? -1),
          key: active?.getAttribute("data-entry-key") ?? "",
          source: active?.querySelector(".src")?.textContent ?? "",
          translation: active?.querySelector(".editor-surface")?.textContent ?? "",
        };
      })()`);
      if (
        state.operation === "reload"
        && state.phase === "cancelled"
        && state.stage === "project.reload.sources"
        && state.operationStatus === "reload: cancelled (project.reload.sources)"
        && state.editorStatus.includes("reload cancelled")
        && !state.cancelVisible
      ) {
        return state;
      }
      throw new Error(JSON.stringify(state));
    },
  );
  const reloadPostCancel = await client.evaluate(`(async () => {
    window.__omegatReloadCancelObserver?.disconnect();
    const entries = await window.omegat.rpc("entry.list", {});
    const version = await window.omegat.rpc("sys.version", {});
    return {
      version,
      entryCount: entries.length,
      firstEntry: {
        key: JSON.stringify(entries[0]?.key ?? null),
        source: entries[0]?.source ?? "",
        translation: entries[0]?.translation ?? "",
      },
      visibleProgress: window.__omegatReloadProgress,
      rpcTrace: window.__omegatRpcOperationTrace,
      domTrace: window.__omegatDomOperationTrace,
    };
  })()`, true);
  const reloadRequestTrace = reloadPostCancel.rpcTrace
    .filter((event) => event.requestId === reloadPostCancel.visibleProgress?.requestId)
    .map((event) => `${event.phase}:${event.stage}`);
  assert(reloadPostCancel.visibleProgress, "reload progress checkpoint was not observed");
  assert.deepEqual(reloadPostCancel.visibleProgress, {
    requestId: reloadPostCancel.visibleProgress.requestId,
    operation: "reload",
    phase: "progress",
    stage: "project.reload.sources",
    status: "reload: project.reload.sources",
    cancelVisible: true,
  });
  assert(reloadPostCancel.visibleProgress.requestId);
  assert.deepEqual(reloadRequestTrace, [
    "started:",
    "progress:project.reload.sources",
    "cancelling:",
    "cancelled:",
  ]);
  assert(
    reloadPostCancel.domTrace.includes(
      "reload|cancelling|project.reload.sources|reload: cancelling (project.reload.sources)",
    ),
    `renderer never visibly entered reload cancelling: ${
      JSON.stringify(reloadPostCancel.domTrace)
    }`,
  );
  assert.deepEqual(
    {
      entry: reloadCancelled.entry,
      key: reloadCancelled.key,
      source: reloadCancelled.source,
      translation: reloadCancelled.translation,
    },
    reloadBefore,
  );
  assert.equal(reloadPostCancel.entryCount, SOURCE_FILES);
  assert.equal(reloadPostCancel.firstEntry.key, reloadBefore.key);
  assert.equal(reloadPostCancel.firstEntry.source, reloadBefore.source);
  assert.equal(reloadPostCancel.firstEntry.translation, reloadBefore.translation);
  assert.equal(reloadPostCancel.version.version, "6.2.0");

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    platform: "linux",
    sourceFiles: SOURCE_FILES,
    compile: {
      visibleProgress,
      cancelled,
      requestTrace,
      domTrace: postCancel.domTrace,
      targetRollback: true,
      residue: [],
    },
    reload: {
      visibleProgress: reloadPostCancel.visibleProgress,
      cancelled: {
        operation: reloadCancelled.operation,
        phase: reloadCancelled.phase,
        stage: reloadCancelled.stage,
        operationStatus: reloadCancelled.operationStatus,
        editorStatus: reloadCancelled.editorStatus,
        cancelVisible: reloadCancelled.cancelVisible,
        entry: reloadCancelled.entry,
      },
      requestTrace: reloadRequestTrace,
      domTrace: reloadPostCancel.domTrace,
      entryRollback: {
        entry: reloadBefore.entry,
        keyPreserved: reloadCancelled.key === reloadBefore.key,
        sourcePreserved: reloadCancelled.source === reloadBefore.source,
        translationPreserved:
          reloadCancelled.translation === reloadBefore.translation,
      },
    },
    sidecarResponsive: postCancel.version.version,
  }));
} catch (error) {
  if (stderr) process.stderr.write(stderr);
  throw error;
} finally {
  client?.close();
  await terminate(application);
  await terminate(xvfb.child);
  await rm(workDir, { recursive: true, force: true });
}
