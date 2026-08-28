// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  mkdtemp,
  open,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

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

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function durableWriteJson(path, value) {
  const handle = await open(path, "w");
  try {
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  const parent = await open(dirname(path), "r");
  try {
    await parent.sync();
  } finally {
    await parent.close();
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

class SidecarSession {
  constructor(configDir) {
    this.child = spawn(sidecar, [], {
      env: { ...process.env, OMEGAT_CONFIG_DIR: configDir },
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.nextId = 1;
    this.pending = new Map();
    this.stdout = "";
    this.stderr = "";
    this.child.stdout.on("data", (chunk) => {
      this.stdout += chunk.toString();
      while (true) {
        const newline = this.stdout.indexOf("\n");
        if (newline < 0) break;
        const line = this.stdout.slice(0, newline).trim();
        this.stdout = this.stdout.slice(newline + 1);
        if (!line) continue;
        const message = JSON.parse(line);
        if (message.id == null) continue;
        const pending = this.pending.get(message.id);
        if (!pending) continue;
        this.pending.delete(message.id);
        if (message.error) {
          pending.reject(new Error(JSON.stringify(message.error)));
        } else {
          pending.resolve(message.result);
        }
      }
    });
    this.child.stderr.on("data", (chunk) => {
      this.stderr += chunk.toString();
    });
  }

  request(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolveRequest, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`sidecar request timed out: ${method}\n${this.stderr}`));
      }, WAIT_MS);
      timeout.unref();
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolveRequest(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          reject(error);
        },
      });
      this.child.stdin.write(
        `${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`,
      );
    });
  }

  async close() {
    if (this.child.exitCode !== null) return;
    const exited = new Promise((resolveExit, reject) => {
      this.child.once("error", reject);
      this.child.once("exit", (code, signal) => resolveExit({ code, signal }));
    });
    this.child.stdin.end();
    const result = await exited;
    assert.equal(result.signal, null, `setup sidecar exited by ${result.signal}`);
    assert.equal(result.code, 0, `setup sidecar failed: ${this.stderr}`);
  }
}

async function workspaceState(client) {
  return client.evaluate(`(() => {
    const app = document.querySelector(".app");
    const segment = document.querySelector(".editor-segment.is-active");
    return {
      project: app?.dataset.projectId ?? null,
      generation: Number(app?.dataset.projectGeneration ?? 0),
      source: segment?.querySelector(".src")?.textContent ?? null,
      translation: segment?.querySelector(".editor-surface")?.textContent ?? null,
      key: segment?.getAttribute("data-entry-key") ?? null,
      activeSurfaces: document.querySelectorAll(
        ".editor-segment.is-active .editor-surface"
      ).length,
    };
  })()`);
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
    const state = await workspaceState(client);
    return state.project === project && state.key ? state : undefined;
  });
  return { application, client, workspace, stderr: () => stderr };
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

async function killPackaged(launched) {
  const processes = await descendants(launched.application.pid);
  const sidecarProcess = processes.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  assert(sidecarProcess, `packaged sidecar not found: ${JSON.stringify(processes)}`);
  const browserPid = launched.application.pid;
  process.kill(-browserPid, "SIGKILL");
  await waitFor("SIGKILLed Electron", async () => !await pathExists(`/proc/${browserPid}`));
  await waitFor("SIGKILLed sidecar", async () =>
    !await pathExists(`/proc/${sidecarProcess.pid}`)
  );
  launched.client.close();
  return { browserPid, sidecarPid: sidecarProcess.pid };
}

async function terminatePackaged(launched) {
  if (!launched?.application?.pid) return;
  launched.client?.close();
  const pid = launched.application.pid;
  try {
    process.kill(-pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  await waitFor("terminated packaged Electron", async () =>
    !await pathExists(`/proc/${pid}`)
  );
}

function parseNdjson(raw) {
  return raw.trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
}

function assertCompleteEntryKey(key) {
  assert.deepEqual(
    Object.keys(key).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
  );
}

async function prepareCompactionProject(configDir, project, label) {
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  const sourcePath = join(project, "source", "source.txt");
  await writeFile(sourcePath, `${label} before compaction`, "utf8");
  await session.request("project.reload", {});
  await writeFile(sourcePath, `${label} committed source`, "utf8");
  const receipt = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 71,
    paths: [sourcePath],
    fingerprints: { [sourcePath]: `${label}-committed` },
    sources: ["native"],
  });
  const receiptBatchId = receipt.batch.batch_id;
  await session.request("project.external-refresh", {
    transaction_project_root: project,
    transaction_generation: 71,
    transaction_batch_id: receiptBatchId,
    app_instance: `${label}-setup`,
  });
  const tail = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 71,
    paths: [sourcePath],
    fingerprints: { [sourcePath]: `${label}-pending-tail` },
    sources: ["sidecar"],
  });
  await session.close();

  const journalPath = join(
    project,
    ".repositories",
    "transactions",
    "external-refresh.json",
  );
  const journal = JSON.parse(await readFile(journalPath, "utf8"));
  assert.equal(journal.batches[0].batch_id, receiptBatchId);
  assert.equal(journal.batches[0].status, "sidecar_committed");
  assert.equal(journal.batches[1].batch_id, tail.batch.batch_id);
  assert.equal(journal.batches[1].status, "pending");
  const terminal = structuredClone(journal.batches[0]);
  terminal.batch_id = `${label}-acknowledged-terminal`;
  terminal.status = "completed";
  terminal.updated_unix_ms = Math.max(1, terminal.updated_unix_ms - 1_000);
  journal.batches.unshift(terminal);
  await durableWriteJson(journalPath, journal);
  return {
    journalPath,
    historyPath: join(
      project,
      ".repositories",
      "transactions",
      "external-refresh-history.ndjson",
    ),
    receiptBatchId,
    tailBatchId: tail.batch.batch_id,
    terminalBatchId: terminal.batch_id,
    source: `${label} committed source`,
    key: journal.batches[1].payload.committed_result.entry_list[0].key,
  };
}

async function prepareProductProject(configDir, project, label) {
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(
    join(project, "source", "source.txt"),
    `${label} isolated source`,
    "utf8",
  );
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 1);
  const translation = `${label} isolated translation 😀`;
  const batchId = `${label}-product-receipt`;
  const committed = await session.request("entry.set", {
    index: entries[0].index,
    key: entries[0].key,
    translation,
    note: "dual packaged recovery",
    revision: entries[0].revision,
    default_translation: false,
    transaction_project_root: project,
    transaction_generation: 81,
    transaction_batch_id: batchId,
  });
  assert.equal(committed.receipt.status, "sidecar_committed");
  await session.close();
  return {
    activePath: join(project, ".repositories", "transactions", "active.json"),
    historyPath: join(project, ".repositories", "transactions", "history.ndjson"),
    batchId,
    source: `${label} isolated source`,
    translation,
    key: entries[0].key,
  };
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises packaged compaction recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-compaction-dual-e2e-"));
const xvfb = await startXvfb();
const results = [];
let launchedA;
let launchedB;

try {
  for (const point of ["after_archive_fsync", "after_queue_rename"]) {
    const scenario = point.replace("after_", "");
    const configA = join(workDir, `${scenario}-config-a`);
    const configB = join(workDir, `${scenario}-config-b`);
    const projectA = join(workDir, `${scenario}-project-a`);
    const projectB = join(workDir, `${scenario}-project-b`);
    const marker = join(workDir, `${scenario}.marker`);
    const preparedA = await prepareCompactionProject(
      configA,
      projectA,
      `${scenario}-A`,
    );
    const preparedB = await prepareProductProject(
      configB,
      projectB,
      `${scenario}-B`,
    );
    assertCompleteEntryKey(preparedA.key);
    assertCompleteEntryKey(preparedB.key);

    [launchedA, launchedB] = await Promise.all([
      launchPackaged(xvfb.display, configA, projectA, {
        OMEGAT_TEST_REFRESH_COMPACTION_POINT: point,
        OMEGAT_TEST_REFRESH_COMPACTION_MARKER: marker,
      }),
      launchPackaged(xvfb.display, configB, projectB),
    ]);
    await waitFor(`${point} durable marker`, () => pathExists(marker));
    assert.equal(launchedA.workspace.source, preparedA.source);
    assert.equal(launchedA.workspace.activeSurfaces, 1);
    assert.equal(launchedB.workspace.source, preparedB.source);
    assert.equal(launchedB.workspace.translation, preparedB.translation);
    assert.equal(launchedB.workspace.activeSurfaces, 1);
    assert.deepEqual(JSON.parse(launchedB.workspace.key), preparedB.key);
    await waitFor("project B recovered renderer acknowledgement", async () =>
      await pathExists(preparedB.activePath) ? undefined : true
    );

    const killedA = await killPackaged(launchedA);
    launchedA = undefined;
    const queueAfterKill = JSON.parse(
      await readFile(preparedA.journalPath, "utf8"),
    );
    const expectedQueue = point === "after_archive_fsync"
      ? [
          preparedA.terminalBatchId,
          preparedA.receiptBatchId,
          preparedA.tailBatchId,
        ]
      : [preparedA.receiptBatchId, preparedA.tailBatchId];
    assert.deepEqual(
      queueAfterKill.batches.map((batch) => batch.batch_id),
      expectedQueue,
    );
    const unacknowledged = queueAfterKill.batches.find((batch) =>
      batch.batch_id === preparedA.receiptBatchId
    );
    const pendingTail = queueAfterKill.batches.find((batch) =>
      batch.batch_id === preparedA.tailBatchId
    );
    assert.equal(unacknowledged.status, "sidecar_committed");
    assert.equal(unacknowledged.commit.manifest_sha256.length, 64);
    assert.equal(pendingTail.status, "pending");
    assert.equal(pendingTail.commit, undefined);

    assert.equal(
      await launchedB.client.evaluate(
        'window.omegat.rpc("sys.version", {}).then((value) => value.version)',
        true,
      ),
      "6.2.0",
      "SIGKILLing project A affected project B",
    );
    const liveB = await workspaceState(launchedB.client);
    assert.equal(liveB.project, projectB);
    assert.equal(liveB.source, preparedB.source);
    assert.equal(liveB.translation, preparedB.translation);
    assert.deepEqual(JSON.parse(liveB.key), preparedB.key);

    launchedA = await launchPackaged(xvfb.display, configA, projectA);
    await waitFor("project A receipt and FIFO tail acknowledgements", async () =>
      await pathExists(preparedA.journalPath) ? undefined : true
    );
    const recoveredA = await workspaceState(launchedA.client);
    assert.equal(recoveredA.project, projectA);
    assert.equal(recoveredA.source, preparedA.source);
    assert.equal(recoveredA.activeSurfaces, 1);
    assert.deepEqual(JSON.parse(recoveredA.key), preparedA.key);
    const entriesA = await launchedA.client.evaluate(
      'window.omegat.rpc("entry.list", {})',
      true,
    );
    assert.equal(entriesA.length, 1);
    assert.equal(entriesA[0].source, preparedA.source);
    assert.deepEqual(entriesA[0].key, preparedA.key);

    const historyA = parseNdjson(await readFile(preparedA.historyPath, "utf8"));
    const completedReceipt = historyA.find((row) =>
      row.batch_id === preparedA.receiptBatchId && row.status === "completed"
    );
    const completedTail = historyA.find((row) =>
      row.batch_id === preparedA.tailBatchId && row.status === "completed"
    );
    assert(completedReceipt, "project A unacknowledged receipt was not recovered");
    assert(completedTail, "project A pending FIFO tail was not dispatched");
    assert.equal(completedReceipt.generation, recoveredA.generation);
    assert.equal(completedTail.generation, recoveredA.generation);
    assert(historyA.every((row) => row.project_root === projectA));

    const historyB = parseNdjson(await readFile(preparedB.historyPath, "utf8"));
    const completedB = historyB.find((row) =>
      row.batch_id === preparedB.batchId
      && row.status === "completed"
      && row.payload.operation === "entry.set"
    );
    assert(completedB, "project B product receipt was not recovered");
    assert.equal(completedB.generation, liveB.generation);
    assert(historyB.every((row) => row.project_root === projectB));
    assert.equal(
      historyB.some((row) =>
        row.batch_id === preparedA.receiptBatchId
        || row.batch_id === preparedA.tailBatchId
      ),
      false,
      "project A receipt entered project B history",
    );
    assert.equal(
      historyA.some((row) => row.batch_id === preparedB.batchId),
      false,
      "project B receipt entered project A history",
    );

    results.push({
      point,
      killedA,
      projectA: {
        generation: recoveredA.generation,
        completeEntryKey: preparedA.key,
        receiptBatchId: preparedA.receiptBatchId,
        pendingTailBatchId: preparedA.tailBatchId,
        queueAfterKill: expectedQueue,
      },
      projectB: {
        generation: liveB.generation,
        completeEntryKey: preparedB.key,
        receiptBatchId: preparedB.batchId,
        remainedResponsive: true,
      },
    });
    await terminatePackaged(launchedA);
    launchedA = undefined;
    await terminatePackaged(launchedB);
    launchedB = undefined;
  }

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    simultaneousElectronInstances: true,
    scenarios: results,
  }));
} catch (error) {
  if (launchedA?.stderr()) process.stderr.write(launchedA.stderr());
  if (launchedB?.stderr()) process.stderr.write(launchedB.stderr());
  throw error;
} finally {
  await terminatePackaged(launchedA);
  await terminatePackaged(launchedB);
  try {
    process.kill(xvfb.child.pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  if (keepWorkDir) {
    process.stderr.write(`Retained packaged E2E work directory: ${workDir}\n`);
  } else {
    await rm(workDir, { recursive: true, force: true });
  }
}
