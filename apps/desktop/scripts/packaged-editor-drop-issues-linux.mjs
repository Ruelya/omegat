// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const WAIT_MS = 30_000;
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
    await sleep(100);
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
    server.close((error) => error ? reject(error) : resolveClose()),
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
  const response = JSON.parse(stdout.trim());
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
    this.events = [];
  }

  async connect() {
    await new Promise((resolveOpen, reject) => {
      this.socket.addEventListener("open", resolveOpen, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id == null) {
        this.events.push(message);
        return;
      }
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
      }, 5_000);
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

async function dispatchFileDrop(client, file) {
  const point = await client.evaluate(`(() => {
    const bounds = document.querySelector(".editor-doc")?.getBoundingClientRect();
    if (!bounds) return null;
    return { x: bounds.left + bounds.width / 2, y: bounds.top + 40 };
  })()`);
  assert(point, "editor drop surface is unavailable");
  const data = {
    items: [],
    files: [file],
    dragOperationsMask: 1,
  };
  for (const type of ["dragEnter", "dragOver", "drop"]) {
    await client.command("Input.dispatchDragEvent", {
      type,
      x: point.x,
      y: point.y,
      data,
    });
  }
}

async function terminate(child) {
  if (!child || child.exitCode != null || child.signalCode != null) return;
  const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
  child.kill("SIGTERM");
  await Promise.race([exited, sleep(2_000)]);
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises file drop and issue location in a Linux package");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-editor-drop-issues-e2e-"));
const configDir = join(workDir, "config");
const initialProject = join(workDir, "initial-project");
const droppedProject = join(workDir, "dropped-project");
const importedFile = join(workDir, "b-imported.txt");
await Promise.all([
  mkdir(configDir, { recursive: true }),
  mkdir(join(initialProject, "source"), { recursive: true }),
  mkdir(join(droppedProject, "source"), { recursive: true }),
]);
await Promise.all([
  writeFile(join(initialProject, "source", "initial.txt"), "Initial project.", "utf8"),
  writeFile(
    join(droppedProject, "source", "a-issue.txt"),
    "Keep <x0/> tag.",
    "utf8",
  ),
  writeFile(importedFile, "Imported by packaged drag.", "utf8"),
]);
await rpcOnce(configDir, "project.create", {
  root: initialProject,
  source_lang: "en",
  target_lang: "fr",
  sentence_seg: false,
});
await rpcOnce(configDir, "project.create", {
  root: droppedProject,
  source_lang: "en",
  target_lang: "fr",
  sentence_seg: false,
});

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
        OMEGAT_PROJECT: initialProject,
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
  await Promise.all([
    client.command("Runtime.enable"),
    client.command("Log.enable"),
  ]);

  const initial = await waitFor("initial editor project", async () => {
    const state = await client.evaluate(`(() => {
      document.querySelectorAll(".modal-bg").forEach((modal) => modal.click());
      return {
        preload: typeof window.omegat?.rpc,
        source: document.querySelector(".editor-segment.is-active .src")?.textContent ?? null,
      };
    })()`);
    if (state.preload === "function" && state.source === "Initial project.") return state;
    throw new Error(JSON.stringify({
      state,
      events: client.events.slice(-5),
    }));
  });
  assert.equal(initial.source, "Initial project.");

  await dispatchFileDrop(client, join(droppedProject, "omegat.project"));
  const opened = await waitFor("project opened through packaged file drop", async () => {
    const state = await client.evaluate(`(async () => ({
      root: [...document.querySelectorAll("footer.status span")].at(-1)?.textContent ?? null,
      source: document.querySelector(".editor-segment.is-active .src")?.textContent ?? null,
      entries: await window.omegat.rpc("entry.list", {})
    }))()`, true);
    if (state.root === droppedProject && state.source === "Keep <x0/> tag.") return state;
    throw new Error(JSON.stringify(state));
  });
  assert.deepEqual(opened.entries.map(({ file, source }) => ({ file, source })), [{
    file: "a-issue.txt",
    source: "Keep <x0/> tag.",
  }]);

  await dispatchFileDrop(client, importedFile);
  const imported = await waitFor("ordinary file imported through packaged drop", async () => {
    const entries = await client.evaluate("window.omegat.rpc('entry.list', {})", true);
    return entries?.length === 2 ? entries : undefined;
  });
  assert.deepEqual(
    imported.map(({ file, source }) => ({ file, source })),
    [
      { file: "a-issue.txt", source: "Keep <x0/> tag." },
      { file: "b-imported.txt", source: "Imported by packaged drag." },
    ],
  );
  assert.equal(
    await readFile(join(droppedProject, "source", "b-imported.txt"), "utf8"),
    "Imported by packaged drag.",
  );

  await client.evaluate(`(() => {
    const surface = document.querySelector(".editor-surface");
    surface?.focus();
    document.querySelector(".ime-proxy")?.focus();
    return document.activeElement?.classList.contains("ime-proxy") ?? false;
  })()`);
  await client.command("Input.insertText", { text: "Traduction sans balise." });
  await waitFor("translation entered through packaged editor", async () => {
    const text = await client.evaluate(
      "document.querySelector('.editor-surface')?.textContent ?? null",
    );
    return text === "Traduction sans balise." ? text : undefined;
  });
  await client.command("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "Enter",
    code: "Enter",
    windowsVirtualKeyCode: 13,
    nativeVirtualKeyCode: 13,
  });
  await client.command("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "Enter",
    code: "Enter",
    windowsVirtualKeyCode: 13,
    nativeVirtualKeyCode: 13,
  });

  const leaveIssue = await waitFor("file-scoped leave issue window", async () => {
    const state = await client.evaluate(`(() => {
      const issue = document.querySelector('[data-issue-kind="tag"]');
      return {
        activeSource: document.querySelector(".editor-segment.is-active .src")?.textContent ?? null,
        issueIndex: issue ? Number(issue.dataset.issueIndex) : null,
        issueFile: issue?.dataset.issueFile ?? null,
        issueText: issue?.textContent?.trim() ?? null,
        modal: Boolean(issue?.closest(".modal")),
      };
    })()`);
    if (state.modal && state.activeSource === "Imported by packaged drag.") return state;
    throw new Error(JSON.stringify(state));
  });
  assert.deepEqual(leaveIssue, {
    activeSource: "Imported by packaged drag.",
    issueIndex: 0,
    issueFile: "a-issue.txt",
    issueText: "tag Tag MISSING",
    modal: true,
  });

  assert.equal(
    await client.evaluate(
      `document.querySelector('[data-issue-kind="tag"]')?.click(); "clicked"`,
    ),
    "clicked",
  );
  const located = await waitFor("issue click located its original entry", async () => {
    const state = await client.evaluate(`({
      activeSource: document.querySelector(".editor-segment.is-active .src")?.textContent ?? null,
      issueOpen: Boolean(document.querySelector('[data-issue-kind="tag"]')),
    })`);
    if (state.activeSource === "Keep <x0/> tag." && !state.issueOpen) return state;
    throw new Error(JSON.stringify(state));
  });
  assert.deepEqual(located, {
    activeSource: "Keep <x0/> tag.",
    issueOpen: false,
  });
  const persisted = await client.evaluate(
    "window.omegat.rpc('entry.get', { index: 0 })",
    true,
  );
  assert.equal(persisted.translation, "Traduction sans balise.");

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    platform: "linux",
    dragInjection: "Chromium Input.dispatchDragEvent with real file paths",
    projectDropRoot: opened.root,
    importedFiles: imported.map(({ file }) => file),
    leaveIssue,
    located,
    translation: persisted.translation,
  }));
  await client.evaluate('setTimeout(() => window.omegat.quit(), 0); "quit"');
} catch (error) {
  if (stderr) process.stderr.write(stderr);
  throw error;
} finally {
  client?.close();
  await terminate(application);
  await terminate(xvfb.child);
  await rm(workDir, { recursive: true, force: true });
}
