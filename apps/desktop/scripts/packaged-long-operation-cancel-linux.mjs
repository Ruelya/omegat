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
        const context = method === "Runtime.evaluate"
          ? ` (${String(params.expression ?? "").replaceAll(/\s+/g, " ").slice(0, 240)})`
          : "";
        reject(new Error(`DevTools command timed out: ${method}${context}`));
      }, WAIT_MS);
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

async function snapshot(root, { ignoreTopLevel = [] } = {}) {
  const ignored = new Set(ignoreTopLevel);
  const files = [];
  async function walk(dir, prefix = "") {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
      if (!prefix && ignored.has(entry.name)) continue;
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
  const payload = "x".repeat(16_384);
  for (let start = 0; start < SOURCE_FILES; start += 100) {
    await Promise.all(
      Array.from(
        { length: Math.min(100, SOURCE_FILES - start) },
        (_, offset) => {
          const index = start + offset;
          if (index === 1_000) {
            return writeFile(
              join(sourceDir, "1000-wanted.yaml"),
              'wanted: "Repeated packaged source"\n',
              "utf8",
            );
          }
          if (index === 1_001) {
            return writeFile(
              join(sourceDir, "1001-decoy.yaml"),
              'decoy: "Repeated packaged source"\n',
              "utf8",
            );
          }
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

async function writeRemoteSources(sourceDir) {
  await mkdir(sourceDir, { recursive: true });
  const payload = "z".repeat(16_384);
  for (let start = 0; start < SOURCE_FILES; start += 100) {
    await Promise.all(
      Array.from(
        { length: Math.min(100, SOURCE_FILES - start) },
        (_, offset) => {
          const index = start + offset;
          return writeFile(
            join(sourceDir, `${String(index).padStart(4, "0")}.txt`),
            `Remote ${index}: ${payload}`,
            "utf8",
          );
        },
      ),
    );
  }
}

function xmlEscape(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function writeAlternativeTmx(path, key, translation) {
  const props = [
    ["file", key.file],
    ["id", key.id],
    ["prev", key.prev],
    ["next", key.next],
    ["path", key.path],
  ]
    .filter(([, value]) => value !== null)
    .map(([name, value]) =>
      `      <prop type="${name}">${xmlEscape(value)}</prop>`
    )
    .join("\n");
  await writeFile(
    path,
    `<?xml version="1.0" encoding="UTF-8"?>
<tmx version="1.4">
  <header creationtool="OmegaT" creationtoolversion="6.2.0" segtype="paragraph" o-tmf="OmegaT TMX" adminlang="EN-US" srclang="en" datatype="plaintext"/>
  <body>
    <tu>
${props}
      <tuv xml:lang="en"><seg>${xmlEscape(key.source_text)}</seg></tuv>
      <tuv xml:lang="fr"><seg>${xmlEscape(translation)}</seg></tuv>
    </tu>
  </body>
</tmx>
`,
    "utf8",
  );
}

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function editorState(client) {
  return client.evaluate(`(() => {
    const segment = document.querySelector(".editor-segment.is-active");
    const surface = segment?.querySelector(".editor-surface");
    const caret = surface?.querySelector(":scope > .caret");
    const following = caret
      ? [...surface.children]
          .slice([...surface.children].indexOf(caret) + 1)
          .find((child) => child.hasAttribute("data-offset"))
      : null;
    return {
      entry: Number(segment?.getAttribute("data-entry") ?? -1),
      key: segment?.getAttribute("data-entry-key") ?? "",
      source: segment?.querySelector(".src")?.textContent ?? "",
      translation: surface?.textContent ?? "",
      caret: following
        ? Number(following.getAttribute("data-offset"))
        : (surface?.textContent.length ?? -1),
    };
  })()`);
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

async function sidecarProjectState(client) {
  return client.evaluate(`(async () => {
    const entries = await window.omegat.rpc("entry.list", {});
    const version = await window.omegat.rpc("sys.version", {});
    return {
      version,
      entryCount: entries.length,
      firstEntry: entries[0] ?? null,
    };
  })()`, true);
}

async function cancelTeamOperation(
  client,
  { kind, action, cancelledMessage, project, remotes },
) {
  const projectBefore = await snapshot(project, {
    ignoreTopLevel: [".repositories"],
  });
  const remoteBefore = await Promise.all(
    remotes.map(({ root }) => snapshot(root)),
  );
  const activeJournal = join(
    project,
    ".repositories",
    "transactions",
    "active.json",
  );
  const activeBefore = await editorState(client);
  const parsedKey = JSON.parse(activeBefore.key);
  assert.deepEqual(
    Object.keys(parsedKey).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
    "the packaged editor did not expose a complete six-field EntryKey",
  );
  const sidecarBefore = await sidecarProjectState(client);
  assert.equal(sidecarBefore.entryCount, SOURCE_FILES);
  assert.equal(sidecarBefore.firstEntry.key.file, parsedKey.file);
  assert.notEqual(
    activeBefore.translation,
    sidecarBefore.firstEntry.translation,
    "the active Document3 must be dirty before team cancellation",
  );

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
    window.__omegatTeamProgress = null;
    window.__omegatTeamCancelObserver?.disconnect();
    const app = document.querySelector(".app");
    window.__omegatTeamCancelObserver = new MutationObserver(() => {
      if (
        app?.dataset.operation === ${JSON.stringify(kind)}
        && app?.dataset.operationPhase === "progress"
        && app?.dataset.operationStage === "team.mapping.copy"
      ) {
        const button = document.querySelector('[data-operation-action="cancel"]');
        if (!button || window.__omegatTeamProgress) return;
        window.__omegatTeamProgress = {
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
    window.__omegatTeamCancelObserver.observe(app, {
      attributes: true,
      subtree: true,
      childList: true,
      characterData: true,
    });
  })()`);

  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector(
        '[data-operation-action=${JSON.stringify(action)}]'
      );
      button?.click();
      return Boolean(button);
    })()`),
    true,
    `visible ${action} action was unavailable`,
  );

  const terminal = await waitFor(`${kind} protocol-confirmed cancellation`, async () => {
    const state = await client.evaluate(`(() => {
      const app = document.querySelector(".app");
      return {
        operation: app?.dataset.operation ?? "",
        phase: app?.dataset.operationPhase ?? "",
        stage: app?.dataset.operationStage ?? "",
        operationStatus: document.querySelector("[data-operation-status]")?.textContent ?? "",
        teamMessage: document.querySelector("[data-team-message]")?.textContent ?? "",
        cancelVisible: Boolean(document.querySelector('[data-operation-action="cancel"]')),
      };
    })()`);
    if (
      state.operation === kind
      && state.phase === "cancelled"
      && state.stage === "team.mapping.copy"
      && state.operationStatus === `${kind}: cancelled (team.mapping.copy)`
      && state.teamMessage === cancelledMessage
      && !state.cancelVisible
    ) {
      return state;
    }
    throw new Error(JSON.stringify(state));
  });

  const postCancel = await client.evaluate(`(async () => {
    window.__omegatTeamCancelObserver?.disconnect();
    const entries = await window.omegat.rpc("entry.list", {});
    const version = await window.omegat.rpc("sys.version", {});
    return {
      visibleProgress: window.__omegatTeamProgress,
      rpcTrace: window.__omegatRpcOperationTrace,
      domTrace: window.__omegatDomOperationTrace,
      version,
      entryCount: entries.length,
      firstEntry: entries[0] ?? null,
    };
  })()`, true);
  assert(postCancel.visibleProgress, `${kind} progress checkpoint was not observed`);
  assert.equal(postCancel.visibleProgress.operation, kind);
  assert.equal(postCancel.visibleProgress.phase, "progress");
  assert.equal(postCancel.visibleProgress.stage, "team.mapping.copy");
  assert.equal(postCancel.visibleProgress.status, `${kind}: team.mapping.copy`);
  assert.equal(postCancel.visibleProgress.cancelVisible, true);
  assert(postCancel.visibleProgress.requestId);

  const requestEvents = postCancel.rpcTrace.filter(
    (event) => event.requestId === postCancel.visibleProgress.requestId,
  );
  const requestTrace = requestEvents
    .map((event) => `${event.phase}:${event.stage}`)
    .filter((value, index, values) => index === 0 || values[index - 1] !== value);
  assert.deepEqual(requestTrace, [
    "started:",
    "progress:team.mapping.copy",
    "cancelling:",
    "cancelled:",
  ]);
  const cancelledEvent = requestEvents.find((event) => event.phase === "cancelled");
  assert.equal(
    cancelledEvent?.errorCode,
    -32800,
    `${kind} became terminal without the sidecar cancellation code`,
  );
  assert(
    postCancel.domTrace.includes(
      `${kind}|cancelling|team.mapping.copy|${kind}: cancelling (team.mapping.copy)`,
    ),
    `renderer never visibly entered ${kind} cancelling: ${
      JSON.stringify(postCancel.domTrace)
    }`,
  );

  assert.deepEqual(
    await snapshot(project, { ignoreTopLevel: [".repositories"] }),
    projectBefore,
  );
  const remoteAfter = await Promise.all(
    remotes.map(({ root }) => snapshot(root)),
  );
  assert.deepEqual(remoteAfter, remoteBefore);
  assert.equal(await pathExists(activeJournal), false);
  assert.deepEqual(await editorState(client), activeBefore);
  assert.equal(postCancel.version.version, "6.2.0");
  assert.equal(postCancel.entryCount, sidecarBefore.entryCount);
  assert.deepEqual(postCancel.firstEntry, sidecarBefore.firstEntry);

  return {
    visibleProgress: postCancel.visibleProgress,
    terminal,
    requestTrace,
    protocolErrorCode: cancelledEvent.errorCode,
    domTrace: postCancel.domTrace,
    projectRollback: true,
    remoteRollback: Object.fromEntries(remotes.map(({ name }) => [name, true])),
    activeJournalRemoved: true,
    editor: {
      entry: activeBefore.entry,
      completeEntryKey: parsedKey,
      translation: activeBefore.translation,
      caret: activeBefore.caret,
    },
    sidecar: {
      version: postCancel.version.version,
      entries: postCancel.entryCount,
    },
  };
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
const mainRemote = join(workDir, "main-file-remote");
const mappingRemote = join(workDir, "mapping-file-remote");
const conflictRemote = join(workDir, "complete-key-conflict-remote");
await mkdir(configDir, { recursive: true });
await rpcOnce(configDir, "project.create", {
  root: project,
  source_lang: "en",
  target_lang: "fr",
  sentence_seg: false,
});
await writeSources(sourceDir);
await mkdir(mainRemote, { recursive: true });
await writeRemoteSources(join(mappingRemote, "source"));
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
        errorCode: event.errorCode ?? null,
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

  const repositories = [
    {
      repo_type: "file",
      url: mainRemote,
      branch: null,
      mappings: [{
        local: "/",
        repository: "/",
        includes: ["main-repository-only.marker"],
        excludes: [],
      }],
    },
    {
      repo_type: "file",
      url: mappingRemote,
      branch: null,
      mappings: [{
        local: "/source/",
        repository: "/source/",
        includes: [],
        excludes: [],
      }],
    },
  ];
  const configured = await client.evaluate(
    `window.omegat.rpc("team.mapping", ${JSON.stringify({ repositories })})`,
    true,
  );
  assert.equal(configured.ok, true);
  assert.equal(configured.repositories.length, 2);

  assert.equal(
    await client.evaluate(`(() => {
      const surface = document.querySelector(".editor-surface");
      surface?.focus();
      return document.activeElement?.classList.contains("ime-proxy") ?? false;
    })()`),
    true,
    "packaged editor did not focus its native input proxy",
  );
  const dirtyTeamTranslation = "unsaved team transaction draft 😀";
  await client.command("Input.insertText", { text: dirtyTeamTranslation });
  await waitFor("dirty team-cancellation Document3", async () => {
    const state = await editorState(client);
    return state.translation === dirtyTeamTranslation ? state : undefined;
  });
  for (let index = 0; index < 6; index += 1) {
    await client.command("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
    await client.command("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
  }
  const dirtyEditor = await editorState(client);
  assert.equal(dirtyEditor.translation, dirtyTeamTranslation);
  // The first ArrowLeft crosses the terminal emoji's UTF-16 surrogate pair
  // atomically; the remaining five cross one code unit each.
  assert.equal(dirtyEditor.caret, dirtyTeamTranslation.length - 7);

  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector('[data-operation-action="team-window"]');
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible team action was unavailable",
  );
  await waitFor("team transaction window", async () =>
    await client.evaluate(
      `Boolean(document.querySelector('[data-operation-action="team-sync"]'))`,
    )
      ? true
      : undefined
  );

  const teamRemotes = [
    { name: "main", root: mainRemote },
    { name: "mapping", root: mappingRemote },
  ];
  const teamSync = await cancelTeamOperation(client, {
    kind: "teamSync",
    action: "team-sync",
    cancelledMessage: "sync cancelled",
    project,
    remotes: teamRemotes,
  });
  const teamCommit = await cancelTeamOperation(client, {
    kind: "teamCommit",
    action: "team-commit-source",
    cancelledMessage: "commit source cancelled",
    project,
    remotes: teamRemotes,
  });
  assert.deepEqual(await editorState(client), dirtyEditor);

  const duplicateSetup = await client.evaluate(`(async () => {
    const entries = await window.omegat.rpc("entry.list", {});
    const wanted = entries.find((entry) => entry.key.file === "1000-wanted.yaml");
    const decoy = entries.find((entry) => entry.key.file === "1001-decoy.yaml");
    if (!wanted || !decoy) throw new Error("packaged duplicate entries were not loaded");
    const translation = "wanted duplicate translation 😀 tail";
    await window.omegat.rpc("entry.set", {
      index: wanted.index,
      key: wanted.key,
      translation,
      note: "wanted duplicate note",
      revision: wanted.revision,
      default_translation: false,
    });
    await window.omegat.rpc("project.save", {});
    return {
      translation,
      wanted: { index: wanted.index, key: wanted.key },
      decoy: { index: decoy.index, key: decoy.key },
    };
  })()`, true);
  assert.equal(
    duplicateSetup.wanted.key.source_text,
    duplicateSetup.decoy.key.source_text,
  );
  assert.notEqual(duplicateSetup.wanted.key.file, duplicateSetup.decoy.key.file);
  assert.notEqual(duplicateSetup.wanted.key.id, duplicateSetup.decoy.key.id);
  assert.notEqual(duplicateSetup.wanted.key.path, duplicateSetup.decoy.key.path);
  assert.deepEqual(
    Object.keys(duplicateSetup.wanted.key).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
  );

  await client.evaluate(`(() => {
    window.prompt = () => ${JSON.stringify(String(duplicateSetup.wanted.index + 1))};
  })()`);
  await xdotool(xvfb.display, ["windowfocus", "--sync", String(windowId)]);
  await xdotool(xvfb.display, ["key", "--clearmodifiers", "ctrl+j"]);
  await waitFor("duplicated-source packaged entry", async () => {
    const state = await editorState(client);
    return state.key === JSON.stringify(duplicateSetup.wanted.key)
      ? state
      : undefined;
  });
  assert.equal(
    await client.evaluate(`(() => {
      const surface = document.querySelector(".editor-segment.is-active .editor-surface");
      surface?.focus();
      return document.activeElement?.classList.contains("ime-proxy") ?? false;
    })()`),
    true,
  );
  assert.equal(
    (await editorState(client)).translation,
    duplicateSetup.translation,
    "packaged duplicate translation was not loaded before editing",
  );
  await xdotool(xvfb.display, ["key", "--clearmodifiers", "ctrl+a"]);
  await waitFor("selected duplicated-source translation", async () =>
    await client.evaluate(
      `Boolean(document.querySelector(".editor-segment.is-active .editor-selection"))`,
    )
      ? true
      : undefined
  );
  await client.command("Input.insertText", {
    text: duplicateSetup.translation,
  });
  await waitFor("dirty duplicated-source Document3", async () => {
    const state = await editorState(client);
    return state.translation === duplicateSetup.translation ? state : undefined;
  });
  for (let index = 0; index < 6; index += 1) {
    await client.command("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
    await client.command("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
  }
  const duplicateBeforeRefresh = await editorState(client);
  assert.equal(
    duplicateBeforeRefresh.caret,
    duplicateSetup.translation.length - 7,
    "UTF-16 caret did not cross the packaged emoji atomically",
  );

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
    window.__omegatExternalChangeTrace = [];
    window.__omegatStopExternalTrace?.();
    window.__omegatStopExternalTrace = window.omegat.onProjectExternalChange((event) => {
      window.__omegatExternalChangeTrace.push(event);
    });
    window.__omegatExternalRefreshProgress = null;
    window.__omegatExternalRefreshCancelObserver?.disconnect();
    const app = document.querySelector(".app");
    window.__omegatExternalRefreshCancelObserver = new MutationObserver(() => {
      if (
        app?.dataset.operation === "externalRefresh"
        && app?.dataset.operationPhase === "progress"
        && app?.dataset.operationStage === "project.external-refresh.sources"
      ) {
        const button = document.querySelector('[data-operation-action="cancel"]');
        if (!button || window.__omegatExternalRefreshProgress) return;
        window.__omegatExternalRefreshProgress = {
          requestId: app.dataset.operationRequestId ?? "",
          phase: app.dataset.operationPhase,
          stage: app.dataset.operationStage,
          status: document.querySelector("[data-operation-status]")?.textContent ?? "",
        };
        button.click();
      }
    });
    window.__omegatExternalRefreshCancelObserver.observe(app, {
      attributes: true,
      subtree: true,
      childList: true,
      characterData: true,
    });
  })()`);
  const reorderPath = join(sourceDir, "0999-reorder.yaml");
  await writeFile(reorderPath, 'reorder: "First external candidate"\n', "utf8");

  const externalRefreshCancelled = await waitFor(
    "protocol-confirmed external refresh cancellation",
    async () => {
      const state = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        return {
          operation: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
          stage: app?.dataset.operationStage ?? "",
          operationStatus: document.querySelector("[data-operation-status]")?.textContent ?? "",
          editorStatus: [...document.querySelectorAll("footer.status span")]
            .map((node) => node.textContent ?? ""),
          cancelVisible: Boolean(document.querySelector('[data-operation-action="cancel"]')),
        };
      })()`);
      if (
        state.operation === "externalRefresh"
        && state.phase === "cancelled"
        && state.stage === "project.external-refresh.sources"
        && state.operationStatus
          === "externalRefresh: cancelled (project.external-refresh.sources)"
        && state.editorStatus.includes("external refresh cancelled")
        && !state.cancelVisible
      ) {
        return state;
      }
      throw new Error(JSON.stringify(state));
    },
  );
  const externalCancelPost = await client.evaluate(`(async () => {
    window.__omegatExternalRefreshCancelObserver?.disconnect();
    const stats = await window.omegat.rpc("stats.get", {});
    const wanted = await window.omegat.rpc("entry.get", {
      index: ${duplicateSetup.wanted.index},
    });
    const decoy = await window.omegat.rpc("entry.get", {
      index: ${duplicateSetup.decoy.index},
    });
    return {
      visibleProgress: window.__omegatExternalRefreshProgress,
      rpcTrace: window.__omegatRpcOperationTrace,
      domTrace: window.__omegatDomOperationTrace,
      externalTrace: window.__omegatExternalChangeTrace,
      entryCount: stats.segments,
      wanted,
      decoy,
    };
  })()`, true);
  const externalCancelEvents = externalCancelPost.rpcTrace.filter(
    (event) => event.requestId === externalCancelPost.visibleProgress?.requestId,
  );
  const externalCancelTrace = externalCancelEvents
    .map((event) => `${event.phase}:${event.stage}`)
    .filter((value, index, values) => index === 0 || values[index - 1] !== value);
  assert.deepEqual(externalCancelTrace, [
    "started:",
    "progress:project.external-refresh.sources",
    "cancelling:",
    "cancelled:",
  ]);
  assert.equal(
    externalCancelEvents.find((event) => event.phase === "cancelled")?.errorCode,
    -32800,
  );
  assert(
    externalCancelPost.domTrace.includes(
      "externalRefresh|cancelling|project.external-refresh.sources|externalRefresh: cancelling (project.external-refresh.sources)",
    ),
  );
  const externalCancelRequests = [
    ...new Set(
      externalCancelPost.rpcTrace
        .filter((event) => event.method === "project.external-refresh")
        .map((event) => event.requestId),
    ),
  ];
  assert.equal(
    externalCancelRequests.length,
    1,
    JSON.stringify({
      externalCancelRequests,
      externalTrace: externalCancelPost.externalTrace,
    }),
  );
  assert.equal(externalCancelPost.entryCount, SOURCE_FILES);
  assert.deepEqual(await editorState(client), duplicateBeforeRefresh);
  assert.deepEqual(externalCancelPost.wanted.key, duplicateSetup.wanted.key);
  assert.equal(externalCancelPost.wanted.translation, duplicateSetup.translation);
  assert.equal(externalCancelPost.decoy.translation, "");

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
  })()`);
  await writeFile(reorderPath, 'reorder: "Committed external candidate"\n', "utf8");
  const externalRefreshSucceeded = await waitFor(
    "successful complete-key external refresh",
    async () => {
      const state = await editorState(client);
      const operation = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        return {
          kind: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
        };
      })()`);
      if (
        operation.kind === "externalRefresh"
        && operation.phase === "succeeded"
        && state.key === JSON.stringify(duplicateSetup.wanted.key)
        && state.entry === duplicateBeforeRefresh.entry + 1
        && state.translation === duplicateSetup.translation
        && state.caret === duplicateBeforeRefresh.caret
      ) {
        return { operation, editor: state };
      }
      throw new Error(JSON.stringify({ operation, state }));
    },
  );
  const externalSuccessPost = await client.evaluate(`(async () => {
    const stats = await window.omegat.rpc("stats.get", {});
    const wanted = await window.omegat.rpc("entry.get", {
      index: ${duplicateSetup.wanted.index + 1},
    });
    const decoy = await window.omegat.rpc("entry.get", {
      index: ${duplicateSetup.decoy.index + 1},
    });
    return {
      events: window.__omegatExternalChangeTrace,
      entryCount: stats.segments,
      wanted,
      decoy,
    };
  })()`, true);
  assert.equal(externalSuccessPost.entryCount, SOURCE_FILES + 1);
  assert.deepEqual(externalSuccessPost.wanted.key, duplicateSetup.wanted.key);
  assert.equal(externalSuccessPost.wanted.translation, duplicateSetup.translation);
  assert.equal(externalSuccessPost.decoy.translation, "");
  assert.deepEqual(
    new Set(
      externalSuccessPost.events.flatMap((event) => event.sources),
    ),
    new Set(["native", "sidecar"]),
    "packaged refresh did not traverse both project.files-changed sources",
  );

  const teamConflictOurs = "packaged ours resolution 😀 tail";
  const teamConflictTheirs = "packaged them resolution 😀 tail";
  assert.equal(
    teamConflictOurs.length,
    teamConflictTheirs.length,
    "ours/theirs fixtures must preserve the UTF-16 caret exactly",
  );
  const conflictRemoteTmx = join(conflictRemote, "omegat", "project_save.tmx");
  await Promise.all([
    mkdir(join(conflictRemote, "omegat"), { recursive: true }),
    mkdir(join(conflictRemote, "source"), { recursive: true }),
  ]);
  await Promise.all([
    writeAlternativeTmx(
      conflictRemoteTmx,
      duplicateSetup.wanted.key,
      duplicateSetup.translation,
    ),
    writeFile(
      join(conflictRemote, "source", "0998-team-order.yaml"),
      'remote_order: "Team remote inserted before duplicate"\n',
      "utf8",
    ),
  ]);
  const conflictRepositories = [{
    repo_type: "file",
    url: conflictRemote,
    branch: null,
    mappings: [
      {
        local: "/source/0998-team-order.yaml",
        repository: "/source/0998-team-order.yaml",
        includes: [],
        excludes: [],
      },
      {
        local: "/omegat/project_save.tmx",
        repository: "/omegat/project_save.tmx",
        includes: [],
        excludes: [],
      },
    ],
  }];
  const conflictConfigured = await client.evaluate(
    `window.omegat.rpc("team.mapping", ${
      JSON.stringify({ repositories: conflictRepositories })
    })`,
    true,
  );
  assert.equal(conflictConfigured.ok, true);
  const beforeTeamOrdering = await editorState(client);
  assert.equal(
    beforeTeamOrdering.key,
    JSON.stringify(duplicateSetup.wanted.key),
  );

  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
  })()`);
  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector('[data-operation-action="team-sync"]');
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible team sync action was unavailable for remote ordering",
  );
  const teamOrdering = await waitFor(
    "remote team insertion and complete-key rebind",
    async () => {
      const state = await editorState(client);
      const ui = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        return {
          operation: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
          teamMessage: document.querySelector("[data-team-message]")?.textContent ?? "",
          conflicts: document.querySelectorAll("[data-team-conflict-key]").length,
        };
      })()`);
      if (
        ui.operation === "externalRefresh"
        && ui.phase === "succeeded"
        && ui.teamMessage.startsWith("sync:")
        && ui.conflicts === 0
        && state.key === JSON.stringify(duplicateSetup.wanted.key)
        && state.entry === beforeTeamOrdering.entry + 1
        && state.translation === duplicateSetup.translation
        && state.caret === beforeTeamOrdering.caret
      ) {
        return { ui, editor: state };
      }
      throw new Error(JSON.stringify({ ui, state }));
    },
  );
  const teamOrderingEntries = await client.evaluate(
    `window.omegat.rpc("entry.list", {})`,
    true,
  );
  const orderedWanted = teamOrderingEntries.find((entry) =>
    JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.wanted.key)
  );
  const orderedDecoy = teamOrderingEntries.find((entry) =>
    JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.decoy.key)
  );
  assert(orderedWanted, "wanted duplicate disappeared after remote ordering");
  assert(orderedDecoy, "decoy duplicate disappeared after remote ordering");
  assert.equal(orderedWanted.index, duplicateSetup.wanted.index + 2);
  assert.equal(orderedDecoy.index, duplicateSetup.decoy.index + 2);
  assert.equal(orderedDecoy.translation, "");
  const teamOrderingTrace = await client.evaluate(
    `window.__omegatRpcOperationTrace`,
  );
  assert(
    teamOrderingTrace.some((event) =>
      event.method === "team.sync" && event.phase === "succeeded"
    ),
    JSON.stringify(teamOrderingTrace),
  );
  assert(
    teamOrderingTrace.some((event) =>
      event.method === "project.external-refresh" && event.phase === "succeeded"
    ),
    JSON.stringify(teamOrderingTrace),
  );

  assert.equal(
    await client.evaluate(`(() => {
      const surface = document.querySelector(".editor-segment.is-active .editor-surface");
      surface?.focus();
      return document.activeElement?.classList.contains("ime-proxy") ?? false;
    })()`),
    true,
  );
  await xdotool(xvfb.display, ["key", "--clearmodifiers", "ctrl+a"]);
  await client.command("Input.insertText", { text: teamConflictOurs });
  await waitFor("dirty packaged team ours Document3", async () => {
    const state = await editorState(client);
    return state.translation === teamConflictOurs ? state : undefined;
  });

  await xdotool(xvfb.display, ["windowfocus", "--sync", String(windowId)]);
  await xdotool(xvfb.display, ["key", "--clearmodifiers", "alt+e"]);
  await xdotool(xvfb.display, ["key", "End"]);
  for (let index = 0; index < 3; index += 1) {
    await xdotool(xvfb.display, ["key", "Up"]);
  }
  await xdotool(xvfb.display, ["key", "Return"]);
  await waitFor("decoy after visible alternative-translation commit", async () => {
    const state = await editorState(client);
    return state.key === JSON.stringify(duplicateSetup.decoy.key)
      ? state
      : undefined;
  });
  const entriesAfterAlternativeCommit = await client.evaluate(
    `window.omegat.rpc("entry.list", {})`,
    true,
  );
  assert.equal(
    entriesAfterAlternativeCommit.find((entry) =>
      JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.wanted.key)
    )?.default_translation,
    false,
  );
  assert.equal(
    entriesAfterAlternativeCommit.find((entry) =>
      JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.decoy.key)
    )?.translation,
    "",
  );
  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector(".topbar button");
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible project save action was unavailable for packaged team ours",
  );
  await waitFor("packaged team ours saved to local TMX", async () =>
    (await readFile(join(project, "omegat", "project_save.tmx"), "utf8"))
        .includes(`<seg>${teamConflictOurs}</seg>`)
      ? true
      : undefined
  );
  await client.evaluate(`(() => {
    window.prompt = () => ${JSON.stringify(String(orderedWanted.index + 1))};
  })()`);
  await xdotool(xvfb.display, ["key", "--clearmodifiers", "ctrl+j"]);
  await waitFor("wanted after exact packaged team commit", async () => {
    const state = await editorState(client);
    return state.key === JSON.stringify(duplicateSetup.wanted.key)
      && state.translation === teamConflictOurs
      ? state
      : undefined;
  });
  assert.equal(
    await client.evaluate(`(() => {
      const surface = document.querySelector(".editor-segment.is-active .editor-surface");
      surface?.focus();
      return document.activeElement?.classList.contains("ime-proxy") ?? false;
    })()`),
    true,
    "packaged team conflict entry did not focus its native input proxy",
  );
  for (let index = 0; index < 6; index += 1) {
    await client.command("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
    await client.command("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: "ArrowLeft",
      code: "ArrowLeft",
      windowsVirtualKeyCode: 37,
    });
  }
  const beforeTeamConflict = await editorState(client);
  assert.equal(beforeTeamConflict.translation, teamConflictOurs);
  assert.equal(beforeTeamConflict.caret, teamConflictOurs.length - 7);
  assert.equal(beforeTeamConflict.entry, teamOrdering.editor.entry);

  await writeAlternativeTmx(
    conflictRemoteTmx,
    duplicateSetup.wanted.key,
    teamConflictTheirs,
  );
  await client.evaluate(`(() => {
    window.__omegatRpcOperationTrace = [];
    window.__omegatDomOperationTrace = [];
  })()`);
  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector('[data-operation-action="team-sync"]');
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible team sync action was unavailable for complete-key conflict",
  );
  const visibleTeamConflict = await waitFor(
    "visible complete-key ours/theirs conflict",
    async () => {
      const state = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        const row = document.querySelector("[data-team-conflict-key]");
        return {
          operation: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
          key: row?.getAttribute("data-team-conflict-key") ?? "",
          text: row?.textContent ?? "",
          oursVisible: Boolean(
            row?.querySelector('[data-operation-action="team-resolve-ours"]')
          ),
          theirsVisible: Boolean(
            row?.querySelector('[data-operation-action="team-resolve-theirs"]')
          ),
          count: document.querySelectorAll("[data-team-conflict-key]").length,
        };
      })()`);
      if (
        state.operation === "teamSync"
        && state.phase === "failed"
        && state.count === 1
        && state.oursVisible
        && state.theirsVisible
      ) {
        return state;
      }
      throw new Error(JSON.stringify(state));
    },
  );
  assert.deepEqual(
    JSON.parse(visibleTeamConflict.key),
    duplicateSetup.wanted.key,
  );
  assert(visibleTeamConflict.text.includes(`ours: ${teamConflictOurs}`));
  assert(visibleTeamConflict.text.includes(`theirs: ${teamConflictTheirs}`));
  assert.deepEqual(
    await editorState(client),
    beforeTeamConflict,
    "failed team rebase polluted the active Document3",
  );
  const entriesDuringConflict = await client.evaluate(
    `window.omegat.rpc("entry.list", {})`,
    true,
  );
  assert.equal(
    entriesDuringConflict.find((entry) =>
      JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.wanted.key)
    )?.translation,
    teamConflictOurs,
  );

  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector(
        '[data-operation-action="team-resolve-theirs"]'
      );
      button?.click();
      return Boolean(button);
    })()`),
    true,
    "visible keep-theirs action was unavailable",
  );
  const resolvedTeamConflict = await waitFor(
    "complete-key packaged keep-theirs write-back",
    async () => {
      const state = await editorState(client);
      const ui = await client.evaluate(`(() => {
        const app = document.querySelector(".app");
        return {
          operation: app?.dataset.operation ?? "",
          phase: app?.dataset.operationPhase ?? "",
          conflicts: document.querySelectorAll("[data-team-conflict-key]").length,
          activeSurfaces: document.querySelectorAll(
            ".editor-segment.is-active .editor-surface"
          ).length,
          teamMessage: document.querySelector("[data-team-message]")?.textContent ?? "",
        };
      })()`);
      if (
        ui.operation === "externalRefresh"
        && ui.phase === "succeeded"
        && ui.conflicts === 0
        && ui.activeSurfaces === 1
        && state.key === JSON.stringify(duplicateSetup.wanted.key)
        && state.entry === beforeTeamConflict.entry
        && state.translation === teamConflictTheirs
        && state.caret === beforeTeamConflict.caret
      ) {
        return { ui, editor: state };
      }
      throw new Error(JSON.stringify({ ui, state }));
    },
  );
  const entriesAfterResolution = await client.evaluate(
    `window.omegat.rpc("entry.list", {})`,
    true,
  );
  const resolvedWanted = entriesAfterResolution.find((entry) =>
    JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.wanted.key)
  );
  const untouchedDecoy = entriesAfterResolution.find((entry) =>
    JSON.stringify(entry.key) === JSON.stringify(duplicateSetup.decoy.key)
  );
  assert.equal(resolvedWanted?.index, orderedWanted.index);
  assert.equal(resolvedWanted?.translation, teamConflictTheirs);
  assert.equal(untouchedDecoy?.index, orderedDecoy.index);
  assert.equal(untouchedDecoy?.translation, "");

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
    team: {
      repositories: repositories.map(({ repo_type, url, mappings }) => ({
        repo_type,
        url,
        mappings,
      })),
      sync: teamSync,
      commitSource: teamCommit,
    },
    externalRefresh: {
      duplicateKeys: {
        wanted: duplicateSetup.wanted.key,
        decoy: duplicateSetup.decoy.key,
      },
      cancelled: externalRefreshCancelled,
      cancelTrace: externalCancelTrace,
      rollbackEntryCount: externalCancelPost.entryCount,
      succeeded: externalRefreshSucceeded,
      committedEntryCount: externalSuccessPost.entryCount,
      sources: [...new Set(
        externalSuccessPost.events.flatMap((event) => event.sources),
      )].sort(),
      decoyTranslation: externalSuccessPost.decoy.translation,
    },
    teamConflict: {
      remoteInsertion: "source/0998-team-order.yaml",
      initialIndex: duplicateSetup.wanted.index,
      reorderedIndex: orderedWanted.index,
      completeEntryKey: duplicateSetup.wanted.key,
      visible: visibleTeamConflict,
      selected: "theirs",
      resolved: resolvedTeamConflict,
      wantedTranslation: resolvedWanted.translation,
      decoyTranslation: untouchedDecoy.translation,
      singleDocument3Surface: resolvedTeamConflict.ui.activeSurfaces,
      utf16Caret: resolvedTeamConflict.editor.caret,
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
