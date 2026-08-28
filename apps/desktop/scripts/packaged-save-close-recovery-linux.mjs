// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const WAIT_MS = 60_000;
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

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
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
  assert(response && !response.error, `sidecar setup failed: ${stdout}`);
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
      }, WAIT_MS);
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

async function descendants(rootPid) {
  const found = [];
  const queue = [rootPid];
  while (queue.length > 0) {
    const pid = queue.shift();
    let children = "";
    try {
      children = await readFile(`/proc/${pid}/task/${pid}/children`, "utf8");
    } catch {
      continue;
    }
    for (const value of children.trim().split(/\s+/).filter(Boolean)) {
      const childPid = Number(value);
      let command = "";
      try {
        command = (await readFile(`/proc/${childPid}/cmdline`, "utf8"))
          .replaceAll("\0", " ");
      } catch {
        // Process exited between procfs reads.
      }
      found.push({ pid: childPid, command });
      queue.push(childPid);
    }
  }
  return found;
}

async function launchPackaged(display, configDir, project, extraEnv = {}) {
  const port = await unusedPort();
  let stderr = "";
  const environment = {
    ...process.env,
    DISPLAY: display,
    OMEGAT_CONFIG_DIR: configDir,
    ...extraEnv,
  };
  delete environment.OMEGAT_PROJECT;
  if (project) environment.OMEGAT_PROJECT = project;
  const application = spawn(
    executable,
    [`--remote-debugging-port=${port}`, "--disable-gpu", "--no-sandbox"],
    {
      detached: true,
      env: environment,
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  application.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const target = await waitFor("packaged renderer", () => pageTarget(port));
  const client = new DevToolsClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.command("Runtime.enable");
  const workspace = await waitFor("project workspace", async () => {
    const value = await client.evaluate(`(() => {
      const app = document.querySelector(".app");
      const segment = document.querySelector(".editor-segment.is-active");
      return {
        project: app?.dataset.projectId || null,
        welcome: document.querySelector(".welcome") !== null,
        translation: segment?.querySelector(".editor-surface")?.textContent ?? null,
        key: segment?.getAttribute("data-entry-key") ?? null,
        activeSurfaces: document.querySelectorAll(
          ".editor-segment.is-active .editor-surface"
        ).length,
      };
    })()`);
    return project
      ? value.project === project && value.key ? value : undefined
      : value.project === null && value.welcome && value.activeSurfaces === 0
        ? value
        : undefined;
  });
  return { application, client, workspace, stderr: () => stderr };
}

async function killPackaged(launched) {
  const processes = await descendants(launched.application.pid);
  const sidecarProcess = processes.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  assert(sidecarProcess, `packaged sidecar not found: ${JSON.stringify(processes)}`);
  const browserPid = launched.application.pid;
  process.kill(-browserPid, "SIGKILL");
  await waitFor("SIGKILLed Electron", () => pathExists(`/proc/${browserPid}`).then((v) => !v));
  await waitFor("SIGKILLed sidecar", () =>
    pathExists(`/proc/${sidecarProcess.pid}`).then((v) => !v)
  );
  launched.client.close();
  return { browserPid, sidecarPid: sidecarProcess.pid };
}

async function terminatePackaged(launched) {
  if (!launched?.application?.pid) return;
  launched.client?.close();
  try {
    process.kill(-launched.application.pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

function faultEnv(operation, point, marker) {
  return {
    OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION: operation,
    OMEGAT_TEST_PRODUCT_TRANSACTION_POINT: point,
    OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER: marker,
  };
}

async function triggerEntrySet(client, translation) {
  const entries = await client.evaluate('window.omegat.rpc("entry.list", {})', true);
  assert.equal(entries.length, 1);
  await client.evaluate(`(() => {
    void window.omegat.rpc("entry.set", ${
      JSON.stringify({
        index: 0,
        key: entries[0].key,
        translation,
        note: "",
        revision: entries[0].revision,
        default_translation: false,
      })
    }).catch(() => {});
  })()`);
  return entries[0].key;
}

async function activeEnvelope(path, status) {
  return waitFor(`${status} product envelope`, async () => {
    if (!await pathExists(path)) return undefined;
    const journal = JSON.parse(await readFile(path, "utf8"));
    const envelopes = Array.isArray(journal.batches) ? journal.batches : [journal];
    return envelopes.find((envelope) => envelope.status === status);
  });
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises packaged save/close recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-save-close-e2e-"));
const configDir = join(workDir, "config");
const project = join(workDir, "project");
const active = join(project, ".repositories", "transactions", "active.json");
const history = join(project, ".repositories", "transactions", "history.ndjson");
const tmx = join(project, "omegat", "project_save.tmx");
const preMarker = join(workDir, "before-receipt.marker");
const postMarker = join(workDir, "after-receipt.marker");
const closeMarker = join(workDir, "close-after-receipt.marker");
const source = "PACKAGED SAVE CLOSE SOURCE";
const afterReceipt = "AFTER RECEIPT TRANSLATION";
const afterClose = "PROJECT CLOSE TRANSLATION";
const xvfb = await startXvfb();
let launched;

try {
  await rpcOnce(configDir, "project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(join(project, "source", "source.txt"), source, "utf8");

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("entry.set", "before_atomic_publish", preMarker),
  );
  assert.equal(launched.workspace.translation, "");
  const completeKey = await triggerEntrySet(launched.client, "MUST ROLL BACK");
  await waitFor("pre-receipt checkpoint", () => pathExists(preMarker));
  const pending = await activeEnvelope(active, "pending");
  assert.equal(pending.payload.operation, "entry.set");
  assert.equal((await readFile(tmx, "utf8")).includes("MUST ROLL BACK"), true);
  const killedBefore = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  assert.equal(launched.workspace.translation, "");
  await waitFor("pending save rollback cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  const killedAfterRollback = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("entry.set", "after_atomic_publish", postMarker),
  );
  await triggerEntrySet(launched.client, afterReceipt);
  await waitFor("post-receipt checkpoint", () => pathExists(postMarker));
  const committed = await activeEnvelope(active, "sidecar_committed");
  assert.equal(committed.payload.operation, "entry.set");
  assert.equal(committed.commit.manifest_sha256.length, 64);
  const killedAfterReceipt = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  assert.equal(launched.workspace.translation, afterReceipt);
  await waitFor("committed save cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  const killedBeforeClose = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("project.close", "after_atomic_publish", closeMarker),
  );
  const staged = await launched.client.evaluate(
    `window.omegat.rpc("script.run", ${
      JSON.stringify({
        index: 0,
        source: `editor.setTranslation('${afterClose}');`,
      })
    })`,
    true,
  );
  assert.equal(staged.translation, afterClose);
  assert.equal(staged.saved, false);
  assert.equal((await readFile(tmx, "utf8")).includes(afterClose), false);
  await launched.client.evaluate(`(() => {
    void window.omegat.rpc("project.close", {}).catch(() => {});
  })()`);
  await waitFor("close receipt checkpoint", () => pathExists(closeMarker));
  const closeEnvelope = await activeEnvelope(active, "sidecar_committed");
  assert.equal(closeEnvelope.payload.operation, "project.close");
  const killedAfterClose = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, null);
  await waitFor("committed close cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  const recoveredCloseWorkspace = await launched.client.evaluate(`(() => ({
    project: document.querySelector(".app")?.dataset.projectId || null,
    welcome: document.querySelector(".welcome") !== null,
    activeSurfaces: document.querySelectorAll(
      ".editor-segment.is-active .editor-surface"
    ).length,
  }))()`);
  assert.deepEqual(recoveredCloseWorkspace, {
    project: null,
    welcome: true,
    activeSurfaces: 0,
  });
  const closeRecoveryShutdown = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  assert.equal(launched.workspace.translation, afterClose);
  assert.deepEqual(JSON.parse(launched.workspace.key), completeKey);
  const rows = (await readFile(history, "utf8"))
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  assert.equal(
    rows.filter((row) =>
      row.batch_id === pending.batch_id && row.status === "cancelled"
    ).length,
    1,
  );
  assert.equal(
    rows.filter((row) =>
      row.batch_id === committed.batch_id && row.status === "completed"
    ).length,
    1,
  );
  assert.equal(
    rows.filter((row) =>
      row.batch_id === closeEnvelope.batch_id && row.status === "completed"
    ).length,
    1,
  );

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    completeEntryKey: completeKey,
    beforeReceipt: {
      batchId: pending.batch_id,
      statusAfterRecovery: "cancelled",
      killed: killedBefore,
      recoveryShutdown: killedAfterRollback,
    },
    afterReceipt: {
      batchId: committed.batch_id,
      translation: afterReceipt,
      killed: killedAfterReceipt,
      nextShutdown: killedBeforeClose,
    },
    projectClose: {
      batchId: closeEnvelope.batch_id,
      translation: afterClose,
      killed: killedAfterClose,
      recoveredClosedWorkspace: recoveredCloseWorkspace,
      recoveryShutdown: closeRecoveryShutdown,
    },
  }));
} catch (error) {
  if (launched?.stderr()) process.stderr.write(launched.stderr());
  throw error;
} finally {
  await terminatePackaged(launched);
  try {
    process.kill(xvfb.child.pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  await rm(workDir, { recursive: true, force: true });
}
