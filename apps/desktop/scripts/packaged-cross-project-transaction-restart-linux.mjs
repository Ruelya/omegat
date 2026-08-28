// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  cp,
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
const desktopDir = resolve(import.meta.dirname, "..");
const executable =
  process.env.OMEGAT_PACKAGED_EXECUTABLE
  ?? join(desktopDir, "release", "linux-unpacked", "omegat-desktop");
const sidecar =
  process.env.OMEGAT_SIDECAR
  ?? resolve(desktopDir, "..", "..", "target", "release", "omegat-sidecar");
const keepWorkDir = process.env.OMEGAT_KEEP_E2E_WORKDIR === "1";
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

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function processExited(pid) {
  return !(await pathExists(`/proc/${pid}`));
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
      if (!Number.isInteger(childPid)) continue;
      let command = "";
      try {
        command = (await readFile(`/proc/${childPid}/cmdline`, "utf8"))
          .replaceAll("\0", " ");
      } catch {
        // The process may have exited between /proc reads.
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
  const application = spawn(
    executable,
    [`--remote-debugging-port=${port}`, "--disable-gpu", "--no-sandbox"],
    {
      detached: true,
      env: {
        ...process.env,
        DISPLAY: display,
        OMEGAT_CONFIG_DIR: configDir,
        OMEGAT_PROJECT: project,
        ...extraEnv,
      },
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  application.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const target = await waitFor(`renderer for ${project}`, () => pageTarget(port));
  const client = new DevToolsClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.command("Runtime.enable");
  const workspace = await waitFor(`workspace for ${project}`, async () => {
    const value = await client.evaluate(`(() => {
      const app = document.querySelector(".app");
      const segment = document.querySelector(".editor-segment.is-active");
      const surface = segment?.querySelector(".editor-surface");
      return {
        project: app?.dataset.projectId ?? null,
        source: segment?.querySelector(".src")?.textContent ?? null,
        translation: surface?.textContent ?? null,
        key: segment?.getAttribute("data-entry-key") ?? null,
      };
    })()`);
    return value.project === project && value.key ? value : undefined;
  });
  return { application, client, workspace, stderr: () => stderr };
}

async function killPackaged(launched) {
  const processes = await descendants(launched.application.pid);
  const sidecarProcess = processes.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  assert(
    sidecarProcess,
    `packaged sidecar not found: ${JSON.stringify(processes)}`,
  );
  const browserPid = launched.application.pid;
  const sidecarPid = sidecarProcess.pid;
  const exited = new Promise((resolveExit) =>
    launched.application.once("exit", resolveExit)
  );
  process.kill(-browserPid, "SIGKILL");
  await Promise.race([exited, sleep(5_000)]);
  await waitFor("SIGKILLed Electron process", () => processExited(browserPid));
  await waitFor("SIGKILLed sidecar process", () => processExited(sidecarPid));
  launched.client.close();
  return { browserPid, sidecarPid };
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

async function copyProductSnapshot(project, snapshot, prep) {
  const product = join(snapshot, "project");
  await mkdir(product, { recursive: true });
  for (const entry of await readdir(project, { withFileTypes: true })) {
    if (entry.name === ".repositories") continue;
    await cp(join(project, entry.name), join(product, entry.name), {
      recursive: entry.isDirectory(),
    });
  }
  await cp(prep, join(snapshot, "prep"), { recursive: true });
}

function parseNdjson(raw) {
  return raw.trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises cross-project transaction recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-cross-project-restart-e2e-"));
const configDir = join(workDir, "config");
const checkpointProject = join(workDir, "checkpoint-project");
const projectA = join(workDir, "project-a");
const projectB = join(workDir, "project-b");
const checkpointSourceBefore = "CHECKPOINT SOURCE BEFORE COMMIT";
const checkpointSourceAfter = "CHECKPOINT SOURCE AFTER SIDECAR COMMIT";
const sourceA = "PROJECT A TRANSACTION SOURCE";
const sourceB = "PROJECT B ISOLATED SOURCE";
const checkpointTrace = join(workDir, "external-refresh-trace.ndjson");
const checkpointJournal = join(
  checkpointProject,
  ".repositories",
  "transactions",
  "active.json",
);
const checkpointHistory = join(
  checkpointProject,
  ".repositories",
  "transactions",
  "history.ndjson",
);
const transactionDir = join(projectA, ".repositories", "transactions");
const prepDir = join(projectA, ".repositories", "prep");
const transactionJournal = join(transactionDir, "active.json");
const transactionHistory = join(transactionDir, "history.ndjson");
const snapshot = join(transactionDir, "packaged-conflict-a.snapshot");
const xvfb = await startXvfb();
let launched;

try {
  await mkdir(configDir, { recursive: true });
  await rpcOnce(configDir, "project.create", {
    root: checkpointProject,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await rpcOnce(configDir, "project.create", {
    root: projectA,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await rpcOnce(configDir, "project.create", {
    root: projectB,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await Promise.all([
    writeFile(
      join(checkpointProject, "source", "checkpoint.txt"),
      checkpointSourceBefore,
      "utf8",
    ),
    writeFile(join(projectA, "source", "a.txt"), sourceA, "utf8"),
    writeFile(join(projectB, "source", "b.txt"), sourceB, "utf8"),
  ]);

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    checkpointProject,
    {
      OMEGAT_TEST_CRASH_AFTER_EXTERNAL_REFRESH_COMMIT: "1",
      OMEGAT_TEST_EXTERNAL_REFRESH_TRACE: checkpointTrace,
    },
  );
  assert.equal(launched.workspace.source, checkpointSourceBefore);
  const checkpointProcesses = await descendants(launched.application.pid);
  const checkpointSidecar = checkpointProcesses.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  assert(
    checkpointSidecar,
    `checkpoint sidecar not found: ${JSON.stringify(checkpointProcesses)}`,
  );
  const checkpointBrowserPid = launched.application.pid;
  await writeFile(
    join(checkpointProject, "source", "checkpoint.txt"),
    checkpointSourceAfter,
    "utf8",
  );
  await waitFor(
    "fault-injected Electron crash after sidecar commit",
    () => processExited(checkpointBrowserPid),
  );
  await waitFor("fault-injected sidecar exit", () =>
    processExited(checkpointSidecar.pid)
  );
  try {
    process.kill(-checkpointBrowserPid, "SIGKILL");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  launched.client.close();
  launched = undefined;
  const checkpointEnvelope = JSON.parse(
    await readFile(checkpointJournal, "utf8"),
  ).batches[0];
  assert.equal(checkpointEnvelope.status, "sidecar_committed");
  const checkpointBatchId = checkpointEnvelope.batch_id;

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    checkpointProject,
    { OMEGAT_TEST_EXTERNAL_REFRESH_TRACE: checkpointTrace },
  );
  assert.equal(launched.workspace.source, checkpointSourceAfter);
  await waitFor("renderer ack of recovered sidecar checkpoint", async () =>
    await pathExists(checkpointJournal) ? undefined : true
  );
  await sleep(250);
  const checkpointTraceRows = (await readFile(checkpointTrace, "utf8"))
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);
  assert.equal(
    checkpointTraceRows.length,
    1,
    "recovered renderer replayed project.external-refresh",
  );
  const checkpointRows = parseNdjson(
    await readFile(checkpointHistory, "utf8"),
  );
  const completedCheckpoint = checkpointRows.find((row) =>
    row.batch_id === checkpointBatchId
  );
  assert.equal(completedCheckpoint?.status, "completed");
  const killedCheckpointRecovery = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, projectA);
  assert.equal(launched.workspace.source, sourceA);
  const entriesA = await launched.client.evaluate(
    'window.omegat.rpc("entry.list", {})',
    true,
  );
  assert.equal(entriesA.length, 1);
  const keyA = entriesA[0].key;
  assert.deepEqual(
    Object.keys(keyA).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
  );

  const conflictA = {
    kind: "tmx",
    source: sourceA,
    ours: "project A ours",
    theirs: "project A theirs",
    message: "interrupted project A conflict",
    entry_key: keyA,
  };
  await mkdir(prepDir, { recursive: true });
  await writeFile(
    join(prepDir, "conflicts.json"),
    JSON.stringify([conflictA], null, 2),
    "utf8",
  );
  const enqueued = await launched.client.evaluate(
    `window.omegat.rpc("project.refresh.enqueue", ${
      JSON.stringify({
        root: projectA,
        app_instance: "packaged-project-a-before-kill",
        generation: 71,
        paths: [join(projectA, "source", "a.txt")],
        fingerprints: { "source/a.txt": "project-a-before-kill" },
        sources: ["native"],
      })
    })`,
    true,
  );
  assert.equal(enqueued.batch.version, 1);
  assert.equal(enqueued.batch.project_root, projectA);
  assert.equal(enqueued.batch.generation, 71);
  assert.equal(enqueued.batch.status, "pending");
  const refreshBatchId = enqueued.batch.batch_id;

  await copyProductSnapshot(projectA, snapshot, prepDir);
  const teamEnvelope = {
    version: 1,
    project_root: projectA,
    generation: 71,
    batch_id: "packaged-conflict-a",
    status: "pending",
    error_code: null,
    updated_unix_ms: Date.now(),
    payload: {
      operation: "resolve-conflict",
      phase: "mutating",
      snapshot,
      prep_existed: true,
      file_remotes: [],
      repository_count: 0,
      rollback_versions: [],
      commit_started: [],
      published: [],
    },
  };
  const unifiedJournal = JSON.parse(
    await readFile(transactionJournal, "utf8"),
  );
  unifiedJournal.batches.push(teamEnvelope);
  await writeFile(
    transactionJournal,
    JSON.stringify(unifiedJournal, null, 2),
    "utf8",
  );
  await writeFile(join(prepDir, "conflicts.json"), "[]", "utf8");
  assert.deepEqual(
    unifiedJournal.batches.map((row) => row.batch_id),
    [refreshBatchId, teamEnvelope.batch_id],
  );

  const killedA = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, projectB);
  const workspaceB = launched.workspace;
  const stateB = await launched.client.evaluate(`(async () => ({
    entries: await window.omegat.rpc("entry.list", {}),
    conflicts: (await window.omegat.rpc("team.conflicts", {})).conflicts,
  }))()`, true);
  assert.equal(launched.workspace.source, sourceB);
  assert.equal(launched.workspace.translation, "");
  assert.equal(stateB.entries.length, 1);
  assert.equal(stateB.entries[0].source, sourceB);
  assert.equal(stateB.entries[0].key.file, "b.txt");
  assert.notDeepEqual(stateB.entries[0].key, keyA);
  assert.deepEqual(stateB.conflicts, []);
  await waitFor("project A refresh cancellation after opening B", async () => {
    if (!await pathExists(transactionJournal)) return undefined;
    const journal = JSON.parse(await readFile(transactionJournal, "utf8"));
    return journal.batches.length === 1
        && journal.batches[0]?.batch_id === teamEnvelope.batch_id
      ? journal
      : undefined;
  });
  const refreshRows = parseNdjson(await readFile(transactionHistory, "utf8"));
  const cancelledRefresh = refreshRows.find((row) =>
    row.batch_id === refreshBatchId
  );
  assert.equal(cancelledRefresh?.project_root, projectA);
  assert.equal(cancelledRefresh?.generation, 71);
  assert.equal(cancelledRefresh?.status, "cancelled");

  const killedB = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, projectA);
  const recoveredA = await launched.client.evaluate(`(async () => ({
    entries: await window.omegat.rpc("entry.list", {}),
    conflicts: (await window.omegat.rpc("team.conflicts", {})).conflicts,
  }))()`, true);
  assert.equal(launched.workspace.source, sourceA);
  assert.equal(recoveredA.entries.length, 1);
  assert.deepEqual(recoveredA.entries[0].key, keyA);
  assert.deepEqual(recoveredA.conflicts, [conflictA]);
  assert.equal(await pathExists(transactionJournal), false);
  assert.equal(await pathExists(snapshot), false);
  const teamRows = parseNdjson(await readFile(transactionHistory, "utf8"));
  const recoveredConflict = teamRows
    .filter((row) => row.batch_id === teamEnvelope.batch_id)
    .at(-1);
  assert.equal(recoveredConflict?.project_root, projectA);
  assert.equal(recoveredConflict?.generation, 71);
  assert.equal(recoveredConflict?.status, "cancelled");
  assert.equal(recoveredConflict?.payload?.phase, "recovered");

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    checkpoint: {
      batchId: checkpointBatchId,
      status: completedCheckpoint.status,
      externalRefreshRequests: checkpointTraceRows.length,
      killedBrowserPid: checkpointBrowserPid,
      killedSidecarPid: checkpointSidecar.pid,
      recoveryShutdown: killedCheckpointRecovery,
    },
    killedA,
    killedB,
    projectB: {
      entryKey: stateB.entries[0].key,
      document3Source: workspaceB.source,
      conflicts: stateB.conflicts.length,
    },
    projectA: {
      recoveredEntryKey: recoveredA.entries[0].key,
      recoveredConflicts: recoveredA.conflicts.length,
      refreshBatchStatus: cancelledRefresh.status,
      conflictBatchStatus: recoveredConflict.status,
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
  if (keepWorkDir) {
    console.error(`kept E2E workdir: ${workDir}`);
  } else {
    await rm(workDir, { recursive: true, force: true });
  }
}
