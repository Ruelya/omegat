// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  mkdir,
  mkdtemp,
  open,
  readdir,
  readFile,
  rm,
  stat,
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
      project: app?.dataset.projectId || null,
      generation: Number(app?.dataset.projectGeneration ?? 0),
      welcome: document.querySelector(".welcome") !== null,
      source: segment?.querySelector(".src")?.textContent ?? null,
      translation: segment?.querySelector(".editor-surface")?.textContent ?? null,
      key: segment?.getAttribute("data-entry-key") ?? null,
      activeSurfaces: document.querySelectorAll(
        ".editor-segment.is-active .editor-surface"
      ).length,
    };
  })()`);
}

async function invokeRpcResult(client, method, params) {
  return client.evaluate(`(() => window.omegat.rpc(
    ${JSON.stringify(method)},
    ${JSON.stringify(params)}
  ).then(
    (value) => ({ resolved: true, value }),
    (error) => ({ resolved: false, error: String(error) })
  ))()`, true);
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
  const target = await waitFor(
    project ? `renderer for ${project}` : "renderer without startup project",
    () => pageTarget(port),
  );
  const client = new DevToolsClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.command("Runtime.enable");
  const workspace = await waitFor(
    project ? `workspace for ${project}` : "closed renderer workspace",
    async () => {
      const state = await workspaceState(client);
      return project
        ? state.project === project && state.key ? state : undefined
        : state.project === null && state.welcome && state.activeSurfaces === 0
          ? state
          : undefined;
    },
  );
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

function productJournalBatches(journal) {
  return Array.isArray(journal.batches) ? journal.batches : [journal];
}

async function waitForDroppedAck(path, batchId, operation) {
  return waitFor(`dropped ${operation} acknowledgement`, async () => {
    if (!await pathExists(path)) return undefined;
    const rows = parseNdjson(await readFile(path, "utf8"));
    return rows.find((row) =>
      row.batch_id === batchId
      && row.operation === operation
      && row.result === "dropped"
    );
  });
}

function assertOrderedDispatch(trace, batchIds, label) {
  const positions = batchIds.map((batchId) =>
    trace.findIndex((row) => row.batch_id === batchId)
  );
  assert(
    positions.every((position) => position >= 0),
    `${label} omitted a receipt: ${JSON.stringify(trace)}`,
  );
  assert(
    positions.every((position, index) =>
      index === 0 || positions[index - 1] < position
    ),
    `${label} violated FIFO: ${JSON.stringify(trace)}`,
  );
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

async function prepareProductCompactionProject(
  configDir,
  project,
  remote,
  label,
) {
  await mkdir(join(remote, "target"), { recursive: true });
  const remotePath = join(remote, "target", "compaction.txt");
  await writeFile(remotePath, `${label} remote before`, "utf8");
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await session.request("team.mapping", {
    repositories: [{
      repo_type: "file",
      url: remote,
      branch: null,
      mappings: [{
        local: "/target/compaction.txt",
        repository: "/target/compaction.txt",
        includes: [],
        excludes: [],
      }],
    }],
  });
  await session.request("team.sync", {});
  const source = `${label} duplicate source`;
  await writeFile(join(project, "source", "a-wanted.txt"), source, "utf8");
  await writeFile(join(project, "source", "z-decoy.txt"), source, "utf8");
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 2);
  const wanted = entries.find((entry) => entry.key.file === "a-wanted.txt");
  const decoy = entries.find((entry) => entry.key.file === "z-decoy.txt");
  assert(wanted);
  assert(decoy);
  assertCompleteEntryKey(wanted.key);
  assertCompleteEntryKey(decoy.key);

  const translation = `${label} product compaction translation 😀`;
  const receiptBatchId = `${label}-entry-receipt`;
  const committed = await session.request("entry.set", {
    index: wanted.index,
    key: wanted.key,
    translation,
    note: "product journal compaction",
    revision: wanted.revision,
    default_translation: false,
    transaction_project_root: project,
    transaction_generation: 121,
    transaction_batch_id: receiptBatchId,
  });
  assert.equal(committed.receipt.status, "sidecar_committed");
  assert.equal(committed.receipt.payload.operation, "entry.set");

  const remoteContent = `${label} remote committed exactly once`;
  await writeFile(join(project, "target", "compaction.txt"), remoteContent, "utf8");
  const teamBatchId = `${label}-team-tail`;
  const team = await session.request("team.commit", {
    which: "target",
    transaction_project_root: project,
    transaction_generation: 121,
    transaction_batch_id: teamBatchId,
  });
  assert.equal(team.receipt.status, "sidecar_committed");
  assert.equal(team.receipt.payload.operation, "commit-target");
  assert.equal(await readFile(remotePath, "utf8"), remoteContent);

  const saveBatchId = `${label}-save-tail`;
  const save = await session.request("project.save", {
    transaction_project_root: project,
    transaction_generation: 121,
    transaction_batch_id: saveBatchId,
  });
  assert.equal(save.receipt.status, "sidecar_committed");
  assert.equal(save.receipt.payload.operation, "project.save");
  await sleep(5);
  const refresh = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 121,
    paths: [join(project, "source", "a-wanted.txt")],
    fingerprints: {
      [join(project, "source", "a-wanted.txt")]: `${label}-refresh-tail`,
    },
    sources: ["native"],
  });
  assert.equal(refresh.batch.status, "pending");
  assert.equal(refresh.batch.payload.operation, "project.external-refresh");
  await session.close();

  const transactions = join(project, ".repositories", "transactions");
  const activePath = join(transactions, "active.json");
  const historyPath = join(transactions, "history.ndjson");
  const ownerPath = join(transactions, "renderer-owner.json");
  const journal = JSON.parse(await readFile(activePath, "utf8"));
  assert.equal(journal.version, 2);
  assert.deepEqual(
    journal.batches.map((row) => [row.batch_id, row.status]),
    [
      [receiptBatchId, "sidecar_committed"],
      [teamBatchId, "sidecar_committed"],
      [saveBatchId, "sidecar_committed"],
    ],
  );
  const terminalBatchId = `${label}-acknowledged-terminal`;
  const terminalSnapshot = join(transactions, `${terminalBatchId}.snapshot`);
  await mkdir(terminalSnapshot, { recursive: true });
  await writeFile(join(terminalSnapshot, "archived"), "terminal\n", "utf8");
  const terminal = structuredClone(journal.batches[0]);
  terminal.batch_id = terminalBatchId;
  terminal.status = "completed";
  terminal.updated_unix_ms = Math.max(1, terminal.updated_unix_ms - 1_000);
  terminal.payload.phase = "renderer-acknowledged";
  terminal.payload.snapshot = terminalSnapshot;
  journal.batches.unshift(terminal);
  await durableWriteJson(activePath, journal);

  return {
    activePath,
    historyPath,
    refreshJournalPath: join(transactions, "external-refresh.json"),
    refreshHistoryPath: join(transactions, "external-refresh-history.ndjson"),
    ownerPath,
    remotePath,
    receiptBatchId,
    teamBatchId,
    saveBatchId,
    refreshBatchId: refresh.batch.batch_id,
    terminalBatchId,
    source,
    translation,
    key: wanted.key,
    decoyKey: decoy.key,
    remoteContent,
  };
}

async function prepareMixedReceiptProject(
  configDir,
  project,
  remote,
  label,
  { refreshBeforeTeam = false } = {},
) {
  await mkdir(join(remote, "source"), { recursive: true });
  await writeFile(
    join(remote, "source", "shared.txt"),
    `${label} initial source`,
    "utf8",
  );
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await session.request("team.mapping", {
    repositories: [{
      repo_type: "file",
      url: remote,
      branch: null,
      mappings: [{
        local: "/source/shared.txt",
        repository: "/source/shared.txt",
        includes: [],
        excludes: [],
      }],
    }],
  });
  await session.request("team.sync", {});
  await session.request("project.reload", {});
  const sourcePath = join(project, "source", "shared.txt");
  const source = `${label} committed source`;
  await writeFile(sourcePath, source, "utf8");
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 1);
  assertCompleteEntryKey(entries[0].key);

  let acknowledgedBeforeBatchId = null;
  if (refreshBeforeTeam) {
    const beforeTeam = await session.request("project.refresh.enqueue", {
      root: project,
      app_instance: `${label}-setup`,
      generation: 91,
      paths: [sourcePath],
      fingerprints: { [sourcePath]: `${label}-before-team` },
      sources: ["native"],
    });
    acknowledgedBeforeBatchId = beforeTeam.batch.batch_id;
    await sleep(5);
  }
  const teamBatchId = `${label}-team-receipt`;
  const team = await session.request("team.commit", {
    which: "source",
    transaction_project_root: project,
    transaction_generation: 91,
    transaction_batch_id: teamBatchId,
  });
  assert.equal(team.receipt.payload.operation, "commit-source");
  await sleep(5);
  const refreshOne = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 91,
    paths: [sourcePath],
    fingerprints: { [sourcePath]: `${label}-refresh-one` },
    sources: ["native"],
  });
  await sleep(5);
  const refreshTwo = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 91,
    paths: [sourcePath],
    fingerprints: { [sourcePath]: `${label}-refresh-two` },
    sources: ["sidecar"],
  });
  await session.close();

  return {
    source,
    key: entries[0].key,
    acknowledgedBeforeBatchId,
    teamBatchId,
    refreshOneBatchId: refreshOne.batch.batch_id,
    refreshTwoBatchId: refreshTwo.batch.batch_id,
    saveBatchId: null,
    activePath: join(project, ".repositories", "transactions", "active.json"),
    teamHistoryPath: join(
      project,
      ".repositories",
      "transactions",
      "history.ndjson",
    ),
    refreshJournalPath: join(
      project,
      ".repositories",
      "transactions",
      "external-refresh.json",
    ),
    refreshHistoryPath: join(
      project,
      ".repositories",
      "transactions",
      "external-refresh-history.ndjson",
    ),
  };
}

async function prepareCloseReceiptProject(configDir, project, remote, label) {
  await mkdir(join(remote, "target"), { recursive: true });
  await writeFile(
    join(remote, "target", "takeover.txt"),
    `${label} remote before`,
    "utf8",
  );
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await session.request("team.mapping", {
    repositories: [{
      repo_type: "file",
      url: remote,
      branch: null,
      mappings: [{
        local: "/target/takeover.txt",
        repository: "/target/takeover.txt",
        includes: [],
        excludes: [],
      }],
    }],
  });
  await session.request("team.sync", {});
  const source = `${label} duplicate source`;
  await writeFile(join(project, "source", "a-wanted.txt"), source, "utf8");
  await writeFile(join(project, "source", "z-decoy.txt"), source, "utf8");
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 2);
  const wanted = entries.find((entry) => entry.key.file === "a-wanted.txt");
  const decoy = entries.find((entry) => entry.key.file === "z-decoy.txt");
  assert(wanted);
  assert(decoy);
  assertCompleteEntryKey(wanted.key);
  assertCompleteEntryKey(decoy.key);
  const translation = `${label} close translation 😀`;
  const setBatchId = `${label}-initial-entry`;
  const saved = await session.request("entry.set", {
    index: wanted.index,
    key: wanted.key,
    translation,
    note: "close receipt matrix",
    revision: wanted.revision,
    default_translation: false,
    transaction_project_root: project,
    transaction_generation: 111,
    transaction_batch_id: setBatchId,
  });
  assert.equal(saved.receipt.payload.operation, "entry.set");
  await session.request("transaction.receipt.ack", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 111,
    batch_id: setBatchId,
    operation: "entry.set",
    outcome: "succeeded",
  });
  await session.close();
  return {
    source,
    translation,
    key: wanted.key,
    decoyKey: decoy.key,
    remotePath: join(remote, "target", "takeover.txt"),
    activePath: join(project, ".repositories", "transactions", "active.json"),
    ownerPath: join(
      project,
      ".repositories",
      "transactions",
      "renderer-owner.json",
    ),
    teamHistoryPath: join(
      project,
      ".repositories",
      "transactions",
      "history.ndjson",
    ),
    refreshJournalPath: join(
      project,
      ".repositories",
      "transactions",
      "external-refresh.json",
    ),
    refreshHistoryPath: join(
      project,
      ".repositories",
      "transactions",
      "external-refresh-history.ndjson",
    ),
  };
}

async function prepareAtomicElectionProject(
  configDir,
  project,
  remote,
  label,
  headKind,
) {
  const session = new SidecarSession(configDir);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  let remotePath = null;
  let remoteContent = null;
  if (headKind === "team") {
    remotePath = join(remote, "target", "atomic.txt");
    await mkdir(dirname(remotePath), { recursive: true });
    await writeFile(remotePath, `${label} remote before`, "utf8");
    await session.request("team.mapping", {
      repositories: [{
        repo_type: "file",
        url: remote,
        branch: null,
        mappings: [{
          local: "/target/atomic.txt",
          repository: "/target/atomic.txt",
          includes: [],
          excludes: [],
        }],
      }],
    });
    await session.request("team.sync", {});
  }

  const source = `${label} duplicate source`;
  await writeFile(join(project, "source", "a-wanted.txt"), source, "utf8");
  await writeFile(join(project, "source", "z-decoy.txt"), source, "utf8");
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 2);
  const wanted = entries.find((entry) => entry.key.file === "a-wanted.txt");
  const decoy = entries.find((entry) => entry.key.file === "z-decoy.txt");
  assert(wanted);
  assert(decoy);
  assertCompleteEntryKey(wanted.key);
  assertCompleteEntryKey(decoy.key);
  const translation = `${label} atomic election translation 😀`;
  const initialBatchId = `${label}-initial-entry`;
  const initial = await session.request("entry.set", {
    index: wanted.index,
    key: wanted.key,
    translation,
    note: "atomic replacement election",
    revision: wanted.revision,
    default_translation: false,
    transaction_project_root: project,
    transaction_generation: 131,
    transaction_batch_id: initialBatchId,
  });
  assert.equal(initial.receipt.payload.operation, "entry.set");
  await session.request("transaction.receipt.ack", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 131,
    batch_id: initialBatchId,
    operation: "entry.set",
    outcome: "succeeded",
  });
  const compacted = await session.request("transaction.receipt.pending", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 131,
  });
  assert.deepEqual(compacted.envelopes, []);

  const headBatchId = `${label}-${headKind}-head`;
  let operation;
  if (headKind === "team") {
    remoteContent = `${label} remote committed exactly once`;
    await writeFile(join(project, "target", "atomic.txt"), remoteContent, "utf8");
    const head = await session.request("team.commit", {
      which: "target",
      transaction_project_root: project,
      transaction_generation: 131,
      transaction_batch_id: headBatchId,
    });
    operation = "commit-target";
    assert.equal(head.receipt.payload.operation, operation);
    assert.equal(await readFile(remotePath, "utf8"), remoteContent);
  } else if (headKind === "save") {
    const head = await session.request("project.save", {
      transaction_project_root: project,
      transaction_generation: 131,
      transaction_batch_id: headBatchId,
    });
    operation = "project.save";
    assert.equal(head.receipt.payload.operation, operation);
  } else {
    assert.equal(headKind, "close");
    const head = await session.request("project.close", {
      transaction_project_root: project,
      transaction_generation: 131,
      transaction_batch_id: headBatchId,
    });
    operation = "project.close";
    assert.equal(head.receipt.payload.operation, operation);
    await session.request("project.open", { root: project });
  }

  await sleep(10);
  const refreshPath = join(project, "glossary", `${label}-tail.txt`);
  await mkdir(dirname(refreshPath), { recursive: true });
  await writeFile(refreshPath, `${label} source\t${label} target\n`, "utf8");
  const refresh = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: `${label}-setup`,
    generation: 131,
    paths: [refreshPath],
    fingerprints: { [refreshPath]: `${label}-refresh-tail` },
    sources: ["native"],
  });
  assert.equal(refresh.batch.status, "pending");
  assert.equal(refresh.batch.payload.operation, "project.external-refresh");
  await session.close();

  const transactions = join(project, ".repositories", "transactions");
  return {
    source,
    translation,
    key: wanted.key,
    decoyKey: decoy.key,
    initialBatchId,
    headBatchId,
    operation,
    refreshBatchId: refresh.batch.batch_id,
    activePath: join(transactions, "active.json"),
    historyPath: join(transactions, "history.ndjson"),
    ownerPath: join(transactions, "renderer-owner.json"),
    refreshJournalPath: join(transactions, "external-refresh.json"),
    refreshHistoryPath: join(transactions, "external-refresh-history.ndjson"),
    remotePath,
    remoteContent,
  };
}

async function snapshotStableProjectTree(root) {
  const snapshot = {};
  const visit = async (directory, prefix = "") => {
    const entries = await readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (
        relative === ".repositories/transactions"
        || relative.startsWith(".repositories/transactions/")
        || relative === "omegat/.lock"
      ) {
        continue;
      }
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(path, relative);
      } else if (entry.isFile()) {
        const metadata = await stat(path, { bigint: true });
        snapshot[relative] = {
          bytes: (await readFile(path)).toString("base64"),
          mtimeNs: metadata.mtimeNs.toString(),
        };
      }
    }
  };
  await visit(root);
  return snapshot;
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises packaged compaction recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-compaction-dual-e2e-"));
const xvfb = await startXvfb();
const results = [];
const productCompactionResults = [];
const receiptAckMatrix = [];
const atomicReplacementElectionResults = [];
let launchedA;
let launchedB;
let mixedReceiptRecovery;
let selectedHeadCrashRecovery;
let closeReceiptRecovery;

try {
  for (const point of ["after_archive_fsync", "after_queue_rename"]) {
    const scenario = `product-${point.replace("after_", "")}`;
    const config = join(workDir, `${scenario}-config`);
    const project = join(workDir, `${scenario}-project`);
    const remote = join(workDir, `${scenario}-remote`);
    const marker = join(workDir, `${scenario}.marker`);
    const restartTracePath = join(workDir, `${scenario}-restart-trace.ndjson`);
    const prepared = await prepareProductCompactionProject(
      config,
      project,
      remote,
      scenario,
    );
    const originalQueue = JSON.parse(await readFile(prepared.activePath, "utf8"));
    const remoteMtimeBefore = (await stat(prepared.remotePath, { bigint: true })).mtimeNs;

    launchedA = await launchPackaged(xvfb.display, config, project, {
      OMEGAT_TEST_PRODUCT_COMPACTION_POINT: point,
      OMEGAT_TEST_PRODUCT_COMPACTION_MARKER: marker,
    });
    await waitFor(`${point} product-journal marker`, () => pathExists(marker));
    const parked = await workspaceState(launchedA.client);
    assert.equal(parked.project, project);
    assert.equal(parked.source, prepared.source);
    assert.equal(parked.translation, prepared.translation);
    assert.equal(parked.activeSurfaces, 1);
    assert.deepEqual(JSON.parse(parked.key), prepared.key);
    const durableOwner = JSON.parse(await readFile(prepared.ownerPath, "utf8"));
    assert.equal(durableOwner.project_root, project);
    assert.equal(durableOwner.process_id, launchedA.application.pid);
    assert.equal(durableOwner.generation, parked.generation);
    assert.equal(typeof durableOwner.claim_id, "string");
    assert(durableOwner.claim_id.length > 0);

    const queueAtBoundary = JSON.parse(await readFile(prepared.activePath, "utf8"));
    const refreshAtBoundary = JSON.parse(
      await readFile(prepared.refreshJournalPath, "utf8"),
    );
    const expectedQueue = point === "after_archive_fsync"
      ? [
          prepared.terminalBatchId,
          prepared.receiptBatchId,
          prepared.teamBatchId,
          prepared.saveBatchId,
        ]
      : [
          prepared.receiptBatchId,
          prepared.teamBatchId,
          prepared.saveBatchId,
        ];
    assert.deepEqual(
      queueAtBoundary.batches.map((row) => row.batch_id),
      expectedQueue,
    );
    if (point === "after_archive_fsync") {
      assert.deepEqual(queueAtBoundary, originalQueue);
    }
    for (const row of queueAtBoundary.batches) {
      if (row.batch_id === prepared.terminalBatchId) {
        assert.equal(row.status, "completed");
      } else {
        assert.equal(row.status, "sidecar_committed");
        assert.equal(row.commit.manifest_sha256.length, 64);
      }
    }
    assert.deepEqual(
      refreshAtBoundary.batches.map((row) => [
        row.batch_id,
        row.status,
        row.payload.operation,
      ]),
      [[prepared.refreshBatchId, "pending", "project.external-refresh"]],
      "refresh tail did not remain behind the parked product head",
    );
    const archivedAtBoundary = parseNdjson(
      await readFile(prepared.historyPath, "utf8"),
    );
    assert.equal(
      archivedAtBoundary.filter((row) =>
        row.batch_id === prepared.terminalBatchId && row.status === "completed"
      ).length,
      1,
      `product ${point} archive is not idempotent`,
    );

    launchedB = await launchPackaged(xvfb.display, config, null);
    const contenderScope = {
      root: project,
      app_instance: `${scenario}-pre-kill-contender`,
      owner_process_id: launchedB.application.pid,
      generation: parked.generation + 1,
    };
    const contenderPending = await invokeRpcResult(
      launchedB.client,
      "transaction.receipt.pending",
      contenderScope,
    );
    assert.equal(
      contenderPending.resolved,
      false,
      `product ${point} contender obtained an envelope before owner SIGKILL`,
    );
    assert.match(contenderPending.error, /locked by another process|owned by live app/);
    const contenderAck = await invokeRpcResult(
      launchedB.client,
      "transaction.receipt.ack",
      {
        ...contenderScope,
        batch_id: prepared.receiptBatchId,
        operation: "entry.set",
        outcome: "succeeded",
      },
    );
    assert.equal(
      contenderAck.resolved,
      false,
      `product ${point} contender acknowledged the live owner's head`,
    );
    assert.match(contenderAck.error, /locked by another process|owned by live app/);
    assert.deepEqual(
      JSON.parse(await readFile(prepared.ownerPath, "utf8")),
      durableOwner,
      "pre-kill contender replaced the durable product owner",
    );
    assert.deepEqual(
      JSON.parse(await readFile(prepared.activePath, "utf8")),
      queueAtBoundary,
      "pre-kill contender changed the product queue",
    );
    assert.deepEqual(
      JSON.parse(await readFile(prepared.refreshJournalPath, "utf8")),
      refreshAtBoundary,
      "pre-kill contender changed the refresh tail",
    );
    assert.equal(
      await launchedB.client.evaluate(
        'window.omegat.rpc("sys.version", {}).then((value) => value.version)',
        true,
      ),
      "6.2.0",
      "rejected pre-kill contender did not remain responsive",
    );

    const stableTreeBeforeRecovery = await snapshotStableProjectTree(project);
    const tmxPath = join(project, "omegat", "project_save.tmx");
    const tmxBeforeRecovery = await readFile(tmxPath);
    const tmxMtimeBeforeRecovery = (await stat(tmxPath, { bigint: true })).mtimeNs;
    const killed = await killPackaged(launchedA);
    launchedA = undefined;

    launchedA = await launchPackaged(xvfb.display, config, project, {
      OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: restartTracePath,
    });
    await waitFor(`${point} product FIFO drain`, async () =>
      !await pathExists(prepared.activePath)
        && !await pathExists(prepared.refreshJournalPath)
        ? true
        : undefined
    );
    const recovered = await workspaceState(launchedA.client);
    assert.equal(recovered.project, project);
    assert.equal(recovered.source, prepared.source);
    assert.equal(recovered.translation, prepared.translation);
    assert.equal(recovered.activeSurfaces, 1);
    assert.deepEqual(JSON.parse(recovered.key), prepared.key);
    const entries = await launchedA.client.evaluate(
      'window.omegat.rpc("entry.list", {})',
      true,
    );
    const wanted = entries.find((entry) => entry.key.file === prepared.key.file);
    const decoy = entries.find((entry) => entry.key.file === prepared.decoyKey.file);
    assert.deepEqual(wanted.key, prepared.key);
    assert.equal(wanted.translation, prepared.translation);
    assert.deepEqual(decoy.key, prepared.decoyKey);
    assert.equal(decoy.translation, "");

    const restartTrace = parseNdjson(await readFile(restartTracePath, "utf8"));
    assertOrderedDispatch(
      restartTrace,
      [
        prepared.receiptBatchId,
        prepared.teamBatchId,
        prepared.saveBatchId,
        prepared.refreshBatchId,
      ],
      `product ${point} recovery`,
    );
    assert.equal(
      restartTrace.some((row) => row.batch_id === prepared.terminalBatchId),
      false,
      "product compaction dispatched an acknowledged terminal row",
    );
    const history = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    for (const batchId of [
      prepared.terminalBatchId,
      prepared.receiptBatchId,
      prepared.teamBatchId,
      prepared.saveBatchId,
    ]) {
      assert.equal(
        history.filter((row) =>
          row.batch_id === batchId && row.status === "completed"
        ).length,
        1,
        `product ${point} duplicated terminal history for ${batchId}`,
      );
    }
    const refreshHistory = parseNdjson(
      await readFile(prepared.refreshHistoryPath, "utf8"),
    );
    assert.equal(
      refreshHistory.filter((row) =>
        row.batch_id === prepared.refreshBatchId && row.status === "completed"
      ).length,
      1,
      `product ${point} duplicated refresh-tail terminal history`,
    );
    assert.deepEqual(
      await snapshotStableProjectTree(project),
      stableTreeBeforeRecovery,
      `product ${point} recovery replayed a stable project write`,
    );
    assert.deepEqual(await readFile(tmxPath), tmxBeforeRecovery);
    assert.equal(
      (await stat(tmxPath, { bigint: true })).mtimeNs,
      tmxMtimeBeforeRecovery,
      `product ${point} recovery replayed the TMX write`,
    );
    assert.equal(await readFile(prepared.remotePath, "utf8"), prepared.remoteContent);
    assert.equal(
      (await stat(prepared.remotePath, { bigint: true })).mtimeNs,
      remoteMtimeBefore,
      `product ${point} recovery replayed the team remote write`,
    );
    const takeoverOwner = JSON.parse(await readFile(prepared.ownerPath, "utf8"));
    assert.equal(takeoverOwner.process_id, launchedA.application.pid);
    assert.notEqual(takeoverOwner.process_id, durableOwner.process_id);
    assert.notEqual(takeoverOwner.claim_id, durableOwner.claim_id);
    assert.equal(takeoverOwner.generation, recovered.generation);

    productCompactionResults.push({
      point,
      killed,
      queueAtBoundary: expectedQueue,
      recoveredDispatchOrder: [
        prepared.receiptBatchId,
        prepared.teamBatchId,
        prepared.saveBatchId,
        prepared.refreshBatchId,
      ],
      archivedTerminalCount: 1,
      preKillContender: {
        browserPid: launchedB.application.pid,
        pendingRejected: true,
        acknowledgementRejected: true,
        remainedResponsive: true,
      },
      completeEntryKey: prepared.key,
      document3Surfaces: recovered.activeSurfaces,
      productTmxReplayed: false,
      teamRemoteWriteReplayed: false,
      replacementOwnerClaimId: takeoverOwner.claim_id,
    });
    await terminatePackaged(launchedA);
    launchedA = undefined;
    await terminatePackaged(launchedB);
    launchedB = undefined;
  }

  for (const point of ["after_archive_fsync", "after_queue_rename"]) {
    const scenario = point.replace("after_", "");
    const sharedConfig = join(workDir, `${scenario}-shared-config`);
    const projectA = join(workDir, `${scenario}-project-a`);
    const projectB = join(workDir, `${scenario}-project-b`);
    const marker = join(workDir, `${scenario}.marker`);
    const preparedA = await prepareCompactionProject(
      sharedConfig,
      projectA,
      `${scenario}-A`,
    );
    const preparedB = await prepareProductProject(
      sharedConfig,
      projectB,
      `${scenario}-B`,
    );
    assertCompleteEntryKey(preparedA.key);
    assertCompleteEntryKey(preparedB.key);

    [launchedA, launchedB] = await Promise.all([
      launchPackaged(xvfb.display, sharedConfig, projectA, {
        OMEGAT_TEST_REFRESH_COMPACTION_POINT: point,
        OMEGAT_TEST_REFRESH_COMPACTION_MARKER: marker,
      }),
      launchPackaged(xvfb.display, sharedConfig, projectB),
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
    const archivedAfterKill = parseNdjson(
      await readFile(preparedA.historyPath, "utf8"),
    ).filter((row) => row.batch_id === preparedA.terminalBatchId);
    assert.equal(
      archivedAfterKill.length,
      1,
      "archive-fsync boundary appended the same terminal batch more than once",
    );

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

    launchedA = await launchPackaged(xvfb.display, sharedConfig, projectA);
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
    assert.equal(
      historyA.filter((row) => row.batch_id === preparedA.terminalBatchId).length,
      1,
      "restart appended the already archived terminal batch again",
    );
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

  const mixedConfig = join(workDir, "mixed-receipt-shared-config");
  const mixedProject = join(workDir, "mixed-receipt-project");
  const mixedRemote = join(workDir, "mixed-receipt-remote");
  const firstTracePath = join(workDir, "mixed-first-envelope-trace.ndjson");
  const firstAckTracePath = join(workDir, "mixed-first-ack-trace.ndjson");
  const restartTracePath = join(workDir, "mixed-restart-envelope-trace.ndjson");
  const mixed = await prepareMixedReceiptProject(
    mixedConfig,
    mixedProject,
    mixedRemote,
    "lost-refresh",
  );
  launchedA = await launchPackaged(xvfb.display, mixedConfig, mixedProject, {
    OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR: "project.external-refresh",
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: firstTracePath,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: firstAckTracePath,
  });
  await waitFor("lost refresh acknowledgement checkpoint", async () => {
    if (await pathExists(mixed.activePath)) return undefined;
    const journal = JSON.parse(await readFile(mixed.refreshJournalPath, "utf8"));
    const head = journal.batches[0];
    if (
      head?.batch_id !== mixed.refreshOneBatchId
      || head.status !== "sidecar_committed"
      || !await pathExists(firstAckTracePath)
    ) return undefined;
    const acknowledgements = parseNdjson(await readFile(firstAckTracePath, "utf8"));
    return acknowledgements.some((row) =>
        row.batch_id === mixed.refreshOneBatchId
        && row.operation === "project.external-refresh"
        && row.result === "dropped"
      )
      ? journal
      : undefined;
  });
  const save = await launchedA.client.evaluate(
    'window.omegat.rpc("project.save", {})',
    true,
  );
  assert.equal(save.receipt.payload.operation, "project.save");
  mixed.saveBatchId = save.receipt.batch_id;
  await waitFor("save receipt behind lost refresh ack", async () => {
    const active = JSON.parse(await readFile(mixed.activePath, "utf8"));
    const receipt = productJournalBatches(active).find((row) =>
      row.batch_id === mixed.saveBatchId
      && row.status === "sidecar_committed"
    );
    return receipt
      ? receipt
      : undefined;
  });
  const firstTrace = parseNdjson(await readFile(firstTracePath, "utf8"));
  assert(
    firstTrace.some((row) => row.batch_id === mixed.teamBatchId),
    "the initial packaged process did not dispatch the team receipt",
  );
  assert(
    firstTrace.some((row) => row.batch_id === mixed.refreshOneBatchId),
    "the initial packaged process did not reach the injected lost refresh ack",
  );
  await killPackaged(launchedA);
  launchedA = undefined;

  const historyBeforeRestart = parseNdjson(
    await readFile(mixed.teamHistoryPath, "utf8"),
  );
  assert.equal(
    historyBeforeRestart.filter((row) =>
      row.batch_id === mixed.teamBatchId
      && row.status === "completed"
      && row.payload.operation === "commit-source"
    ).length,
    1,
    "the already acknowledged team receipt lacks one exact terminal record",
  );

  launchedA = await launchPackaged(xvfb.display, mixedConfig, mixedProject, {
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: restartTracePath,
  });
  await waitFor("mixed receipt FIFO drained after restart", async () =>
    !await pathExists(mixed.activePath)
      && !await pathExists(mixed.refreshJournalPath)
      ? true
      : undefined
  );
  const recoveredMixed = await workspaceState(launchedA.client);
  assert.equal(recoveredMixed.project, mixedProject);
  assert.equal(recoveredMixed.source, mixed.source);
  assert.equal(recoveredMixed.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(recoveredMixed.key), mixed.key);
  assert.equal(
    await readFile(join(mixedRemote, "source", "shared.txt"), "utf8"),
    mixed.source,
  );

  const restartTrace = parseNdjson(await readFile(restartTracePath, "utf8"));
  assert.equal(
    restartTrace.some((row) => row.batch_id === mixed.teamBatchId),
    false,
    "restart replayed the already acknowledged team receipt",
  );
  const restartPositions = [
    mixed.refreshOneBatchId,
    mixed.refreshTwoBatchId,
    mixed.saveBatchId,
  ].map((batchId) => restartTrace.findIndex((row) => row.batch_id === batchId));
  assert(
    restartPositions.every((position) => position >= 0),
    `restart did not dispatch every unacknowledged receipt: ${JSON.stringify(restartTrace)}`,
  );
  assert(
    restartPositions[0] < restartPositions[1]
      && restartPositions[1] < restartPositions[2],
    `restart violated refresh/refresh/save FIFO: ${JSON.stringify(restartTrace)}`,
  );

  const refreshHistory = parseNdjson(
    await readFile(mixed.refreshHistoryPath, "utf8"),
  );
  for (const batchId of [
    mixed.refreshOneBatchId,
    mixed.refreshTwoBatchId,
  ]) {
    assert.equal(
      refreshHistory.filter((row) =>
        row.batch_id === batchId && row.status === "completed"
      ).length,
      1,
      `refresh terminal history is not idempotent for ${batchId}`,
    );
  }
  const teamHistory = parseNdjson(
    await readFile(mixed.teamHistoryPath, "utf8"),
  );
  assert.equal(
    teamHistory.filter((row) =>
      row.batch_id === mixed.teamBatchId
      && row.status === "completed"
      && row.payload.operation === "commit-source"
    ).length,
    1,
    "restart duplicated the already acknowledged team terminal record",
  );
  assert.equal(
    teamHistory.filter((row) =>
      row.batch_id === mixed.saveBatchId
      && row.status === "completed"
      && row.payload.operation === "project.save"
    ).length,
    1,
    "restart did not acknowledge the trailing save receipt exactly once",
  );
  mixedReceiptRecovery = {
    lostAckBatchId: mixed.refreshOneBatchId,
    restartedDispatchOrder: [
      mixed.refreshOneBatchId,
      mixed.refreshTwoBatchId,
      mixed.saveBatchId,
    ],
    alreadyAcknowledgedTeamWasNotReplayed: true,
    completeEntryKey: mixed.key,
  };
  receiptAckMatrix.push({
    receiptType: "refresh",
    lostAckBatchId: mixed.refreshOneBatchId,
    notReplayed: [mixed.teamBatchId],
    restartedDispatchOrder: [
      mixed.refreshOneBatchId,
      mixed.refreshTwoBatchId,
      mixed.saveBatchId,
    ],
    trailingReceiptsDrained: true,
  });
  await terminatePackaged(launchedA);
  launchedA = undefined;

  const teamConfig = join(workDir, "lost-team-config");
  const teamProject = join(workDir, "lost-team-project");
  const teamRemote = join(workDir, "lost-team-remote");
  const teamFirstTracePath = join(workDir, "lost-team-first-envelope-trace.ndjson");
  const teamFirstAckTracePath = join(workDir, "lost-team-first-ack-trace.ndjson");
  const teamRestartTracePath = join(workDir, "lost-team-restart-envelope-trace.ndjson");
  const lostTeam = await prepareMixedReceiptProject(
    teamConfig,
    teamProject,
    teamRemote,
    "lost-team",
    { refreshBeforeTeam: true },
  );
  assert(lostTeam.acknowledgedBeforeBatchId);
  const teamRemotePath = join(teamRemote, "source", "shared.txt");
  const teamRemoteBefore = await readFile(teamRemotePath, "utf8");
  const teamRemoteMtimeBefore = (await stat(teamRemotePath, { bigint: true })).mtimeNs;
  launchedA = await launchPackaged(xvfb.display, teamConfig, teamProject, {
    OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR: "commit-source",
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: teamFirstTracePath,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: teamFirstAckTracePath,
  });
  await waitForDroppedAck(
    teamFirstAckTracePath,
    lostTeam.teamBatchId,
    "commit-source",
  );
  const lostTeamActive = JSON.parse(await readFile(lostTeam.activePath, "utf8"));
  const lostTeamReceipt = productJournalBatches(lostTeamActive)[0];
  assert.equal(lostTeamReceipt.batch_id, lostTeam.teamBatchId);
  assert.equal(lostTeamReceipt.status, "sidecar_committed");
  const teamQueueBeforeKill = JSON.parse(
    await readFile(lostTeam.refreshJournalPath, "utf8"),
  );
  assert.deepEqual(
    teamQueueBeforeKill.batches.map((batch) => [batch.batch_id, batch.status]),
    [
      [lostTeam.refreshOneBatchId, "pending"],
      [lostTeam.refreshTwoBatchId, "pending"],
    ],
  );
  await killPackaged(launchedA);
  launchedA = undefined;

  launchedA = await launchPackaged(xvfb.display, teamConfig, teamProject, {
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: teamRestartTracePath,
  });
  await waitFor("team lost-ack FIFO drained after restart", async () =>
    !await pathExists(lostTeam.activePath)
      && !await pathExists(lostTeam.refreshJournalPath)
      ? true
      : undefined
  );
  const recoveredTeam = await workspaceState(launchedA.client);
  assert.equal(recoveredTeam.project, teamProject);
  assert.equal(recoveredTeam.source, lostTeam.source);
  assert.equal(recoveredTeam.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(recoveredTeam.key), lostTeam.key);
  const teamRestartTrace = parseNdjson(
    await readFile(teamRestartTracePath, "utf8"),
  );
  assert.equal(
    teamRestartTrace.some((row) =>
      row.batch_id === lostTeam.acknowledgedBeforeBatchId
    ),
    false,
    "restart replayed the refresh acknowledged before the lost team ack",
  );
  assertOrderedDispatch(
    teamRestartTrace,
    [
      lostTeam.teamBatchId,
      lostTeam.refreshOneBatchId,
      lostTeam.refreshTwoBatchId,
    ],
    "lost team acknowledgement restart",
  );
  const lostTeamHistory = parseNdjson(
    await readFile(lostTeam.teamHistoryPath, "utf8"),
  );
  assert.equal(
    lostTeamHistory.filter((row) =>
      row.batch_id === lostTeam.teamBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "lost team acknowledgement produced more than one terminal ack",
  );
  const lostTeamRefreshHistory = parseNdjson(
    await readFile(lostTeam.refreshHistoryPath, "utf8"),
  );
  for (const batchId of [
    lostTeam.acknowledgedBeforeBatchId,
    lostTeam.refreshOneBatchId,
    lostTeam.refreshTwoBatchId,
  ]) {
    assert.equal(
      lostTeamRefreshHistory.filter((row) =>
        row.batch_id === batchId && row.status === "completed"
      ).length,
      1,
      `team lost-ack scenario duplicated refresh terminal ${batchId}`,
    );
  }
  assert.equal(await readFile(teamRemotePath, "utf8"), teamRemoteBefore);
  assert.equal(
    (await stat(teamRemotePath, { bigint: true })).mtimeNs,
    teamRemoteMtimeBefore,
    "recovering the selected team receipt replayed the remote write",
  );
  receiptAckMatrix.push({
    receiptType: "team",
    lostAckBatchId: lostTeam.teamBatchId,
    notReplayed: [lostTeam.acknowledgedBeforeBatchId],
    restartedDispatchOrder: [
      lostTeam.teamBatchId,
      lostTeam.refreshOneBatchId,
      lostTeam.refreshTwoBatchId,
    ],
    trailingReceiptsDrained: true,
  });
  await terminatePackaged(launchedA);
  launchedA = undefined;

  const saveConfig = join(workDir, "lost-save-config");
  const saveProject = join(workDir, "lost-save-project");
  const saveRemote = join(workDir, "lost-save-remote");
  const saveFirstTracePath = join(workDir, "lost-save-first-envelope-trace.ndjson");
  const saveFirstAckTracePath = join(workDir, "lost-save-first-ack-trace.ndjson");
  const saveRestartTracePath = join(workDir, "lost-save-restart-envelope-trace.ndjson");
  const lostSave = await prepareMixedReceiptProject(
    saveConfig,
    saveProject,
    saveRemote,
    "lost-save",
  );
  launchedA = await launchPackaged(xvfb.display, saveConfig, saveProject, {
    OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR: "project.save",
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: saveFirstTracePath,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: saveFirstAckTracePath,
  });
  await waitFor("pre-save receipts drained", async () =>
    !await pathExists(lostSave.activePath)
      && !await pathExists(lostSave.refreshJournalPath)
      ? true
      : undefined
  );
  const savedWithLostAck = await launchedA.client.evaluate(
    'window.omegat.rpc("project.save", {})',
    true,
  );
  assert.equal(savedWithLostAck.receipt.payload.operation, "project.save");
  lostSave.saveBatchId = savedWithLostAck.receipt.batch_id;
  await waitForDroppedAck(
    saveFirstAckTracePath,
    lostSave.saveBatchId,
    "project.save",
  );
  const lostSaveActive = JSON.parse(await readFile(lostSave.activePath, "utf8"));
  const lostSaveReceipt = productJournalBatches(lostSaveActive)[0];
  assert.equal(lostSaveReceipt.batch_id, lostSave.saveBatchId);
  assert.equal(lostSaveReceipt.status, "sidecar_committed");

  await mkdir(join(saveProject, "glossary"), { recursive: true });
  await writeFile(
    join(saveProject, "glossary", "tail.txt"),
    "tail source\ttail target\n",
    "utf8",
  );
  await waitFor("refresh tail behind lost save ack", async () => {
    if (!await pathExists(lostSave.refreshJournalPath)) return undefined;
    const journal = JSON.parse(await readFile(lostSave.refreshJournalPath, "utf8"));
    return journal.batches.some((batch) =>
        batch.status === "pending"
        && batch.payload.paths.some((path) => path.includes("glossary"))
      )
      ? journal
      : undefined;
  });
  await sleep(300);
  const saveQueueBeforeKill = JSON.parse(
    await readFile(lostSave.refreshJournalPath, "utf8"),
  );
  const saveTailBatchIds = saveQueueBeforeKill.batches
    .filter((batch) => ["pending", "sidecar_committed"].includes(batch.status))
    .map((batch) => batch.batch_id);
  assert(saveTailBatchIds.length > 0);
  await killPackaged(launchedA);
  launchedA = undefined;

  launchedA = await launchPackaged(xvfb.display, saveConfig, saveProject, {
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: saveRestartTracePath,
  });
  await waitFor("save lost-ack FIFO drained after restart", async () =>
    !await pathExists(lostSave.activePath)
      && !await pathExists(lostSave.refreshJournalPath)
      ? true
      : undefined
  );
  const recoveredSave = await workspaceState(launchedA.client);
  assert.equal(recoveredSave.project, saveProject);
  assert.equal(recoveredSave.source, lostSave.source);
  assert.equal(recoveredSave.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(recoveredSave.key), lostSave.key);
  const saveRestartTrace = parseNdjson(
    await readFile(saveRestartTracePath, "utf8"),
  );
  for (const batchId of [
    lostSave.teamBatchId,
    lostSave.refreshOneBatchId,
    lostSave.refreshTwoBatchId,
  ]) {
    assert.equal(
      saveRestartTrace.some((row) => row.batch_id === batchId),
      false,
      `save restart replayed already acknowledged receipt ${batchId}`,
    );
  }
  assertOrderedDispatch(
    saveRestartTrace,
    [lostSave.saveBatchId, ...saveTailBatchIds],
    "lost save acknowledgement restart",
  );
  const lostSaveTeamHistory = parseNdjson(
    await readFile(lostSave.teamHistoryPath, "utf8"),
  );
  assert.equal(
    lostSaveTeamHistory.filter((row) =>
      row.batch_id === lostSave.saveBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "lost save acknowledgement produced more than one terminal ack",
  );
  const lostSaveRefreshHistory = parseNdjson(
    await readFile(lostSave.refreshHistoryPath, "utf8"),
  );
  for (const batchId of saveTailBatchIds) {
    assert.equal(
      lostSaveRefreshHistory.filter((row) =>
        row.batch_id === batchId && row.status === "completed"
      ).length,
      1,
      `save lost-ack scenario duplicated refresh terminal ${batchId}`,
    );
  }
  receiptAckMatrix.push({
    receiptType: "save",
    lostAckBatchId: lostSave.saveBatchId,
    notReplayed: [
      lostSave.teamBatchId,
      lostSave.refreshOneBatchId,
      lostSave.refreshTwoBatchId,
    ],
    restartedDispatchOrder: [lostSave.saveBatchId, ...saveTailBatchIds],
    trailingReceiptsDrained: true,
  });
  await terminatePackaged(launchedA);
  launchedA = undefined;

  const closeConfig = join(workDir, "lost-close-config");
  const closeProject = join(workDir, "lost-close-project");
  const closeRemote = join(workDir, "lost-close-remote");
  const closeFirstTracePath = join(workDir, "close-first-envelope-trace.ndjson");
  const closeFirstAckTracePath = join(workDir, "close-first-ack-trace.ndjson");
  const closeClaimedOwnerTracePath = join(
    workDir,
    "close-claimed-owner-envelope-trace.ndjson",
  );
  const closeRestartTracePath = join(workDir, "close-restart-envelope-trace.ndjson");
  const closeContenderTracePath = join(
    workDir,
    "close-contender-envelope-trace.ndjson",
  );
  const closeHeadMarkerPath = join(workDir, "close-selected-head-sidecar-kill.json");
  const closeOwnerMarkerPath = join(workDir, "close-owner-claim.json");
  const closeDeadOwnerReleasePath = join(workDir, "close-dead-owner-release");
  const closeTakeoverMarkerPath = join(workDir, "close-takeover-claim.json");
  const closeTakeoverReleasePath = join(workDir, "close-takeover-release");
  const lostClose = await prepareCloseReceiptProject(
    closeConfig,
    closeProject,
    closeRemote,
    "lost-close",
  );
  launchedA = await launchPackaged(xvfb.display, closeConfig, closeProject, {
    OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR: "project.close",
    OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_FOR: "project.close",
    OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_MARKER: closeHeadMarkerPath,
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: closeFirstTracePath,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: closeFirstAckTracePath,
  });
  assert.equal(launchedA.workspace.translation, lostClose.translation);
  assert.equal(launchedA.workspace.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(launchedA.workspace.key), lostClose.key);
  const closeRequest = await launchedA.client.evaluate(
    'window.omegat.rpc("project.close", {})',
    true,
  );
  assert.equal(closeRequest.ok, true);
  assert.equal(closeRequest.receipt.payload.operation, "project.close");
  const closeBatchId = closeRequest.receipt.batch_id;
  const closeSelectedMarker = await waitFor(
    "selected close head sidecar SIGKILL",
    async () =>
      await pathExists(closeHeadMarkerPath)
        ? JSON.parse(await readFile(closeHeadMarkerPath, "utf8"))
        : undefined,
  );
  assert.equal(closeSelectedMarker.batch_id, closeBatchId);
  assert.equal(closeSelectedMarker.operation, "project.close");
  assert.equal(closeSelectedMarker.signal, "SIGKILL");
  await waitForDroppedAck(
    closeFirstAckTracePath,
    closeBatchId,
    "project.close",
  );
  const replacementAfterCloseHead = await waitFor(
    "replacement sidecar after selected close head",
    async () => {
      const processes = await descendants(launchedA.application.pid);
      return processes.find(({ command, pid }) =>
        command.includes("omegat-sidecar")
        && pid !== closeSelectedMarker.sidecar_pid
      );
    },
  );
  const closedBeforeKill = await waitFor(
    "closed renderer after lost close acknowledgement",
    async () => {
      const state = await workspaceState(launchedA.client);
      return state.project === null
          && state.welcome
          && state.activeSurfaces === 0
        ? state
        : undefined;
    },
  );
  assert.equal(closedBeforeKill.key, null);
  assert.equal(closedBeforeKill.translation, null);
  let closeJournal = JSON.parse(await readFile(lostClose.activePath, "utf8"));
  let closeBatches = productJournalBatches(closeJournal);
  assert.equal(closeBatches[0].batch_id, closeBatchId);
  assert.equal(closeBatches[0].status, "sidecar_committed");
  assert.equal(closeBatches[0].payload.operation, "project.close");

  const closeTailSession = new SidecarSession(closeConfig);
  await closeTailSession.request("project.open", { root: closeProject });
  const closeTeamBatchId = "lost-close-team-tail";
  await writeFile(
    join(closeProject, "target", "takeover.txt"),
    "close tail committed exactly once",
    "utf8",
  );
  const closeTeam = await closeTailSession.request("team.commit", {
    which: "target",
    transaction_project_root: closeProject,
    transaction_generation: closeBatches[0].generation,
    transaction_batch_id: closeTeamBatchId,
  });
  assert.equal(closeTeam.receipt.batch_id, closeTeamBatchId);
  assert.equal(closeTeam.receipt.payload.operation, "commit-target");
  const closeSaveBatchId = "lost-close-save-tail";
  const closeSave = await closeTailSession.request("project.save", {
    transaction_project_root: closeProject,
    transaction_generation: closeBatches[0].generation,
    transaction_batch_id: closeSaveBatchId,
  });
  assert.equal(closeSave.receipt.batch_id, closeSaveBatchId);
  assert.equal(closeSave.receipt.payload.operation, "project.save");
  closeJournal = JSON.parse(await readFile(lostClose.activePath, "utf8"));
  closeBatches = productJournalBatches(closeJournal);
  assert.deepEqual(
    closeBatches.map((row) => [row.batch_id, row.status]),
    [
      [closeBatchId, "sidecar_committed"],
      [closeTeamBatchId, "sidecar_committed"],
      [closeSaveBatchId, "sidecar_committed"],
    ],
  );
  const closeRemoteBeforeRecovery = await readFile(lostClose.remotePath, "utf8");
  const closeRemoteMtimeBeforeRecovery = (
    await stat(lostClose.remotePath, { bigint: true })
  ).mtimeNs;
  const closeTailPath = join(closeProject, "glossary", "after-close.txt");
  await mkdir(dirname(closeTailPath), { recursive: true });
  await writeFile(
    closeTailPath,
    "after close source\tafter close target\n",
    "utf8",
  );
  const closeTail = await closeTailSession.request("project.refresh.enqueue", {
    root: closeProject,
    app_instance: "lost-close-tail-setup",
    generation: closeBatches[0].generation,
    paths: [closeTailPath],
    fingerprints: { [closeTailPath]: "lost-close-refresh-tail" },
    sources: ["native"],
  });
  const closeTailBatchId = closeTail.batch.batch_id;
  await closeTailSession.close();
  const closeQueueBeforeKill = JSON.parse(
    await readFile(lostClose.refreshJournalPath, "utf8"),
  );
  assert.deepEqual(
    closeQueueBeforeKill.batches.map((batch) => [batch.batch_id, batch.status]),
    [[closeTailBatchId, "pending"]],
  );
  const closeFirstTrace = parseNdjson(
    await readFile(closeFirstTracePath, "utf8"),
  );
  assert.equal(closeFirstTrace[0]?.batch_id, closeBatchId);
  assert.equal(
    closeFirstTrace.some((row) =>
      [closeTeamBatchId, closeSaveBatchId, closeTailBatchId].includes(row.batch_id)
    ),
    false,
    "a team/save/refresh tail bypassed the unacknowledged close receipt",
  );
  const stableTreeBeforeRecovery = await snapshotStableProjectTree(closeProject);
  const killedAfterLostClose = await killPackaged(launchedA);
  launchedA = undefined;

  launchedA = await launchPackaged(
    xvfb.display,
    closeConfig,
    null,
    {
      OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: closeClaimedOwnerTracePath,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: "project.close",
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: closeOwnerMarkerPath,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: closeDeadOwnerReleasePath,
    },
  );
  const ownerClaim = await waitFor("durable close owner claim", async () =>
    await pathExists(closeOwnerMarkerPath)
      ? JSON.parse(await readFile(closeOwnerMarkerPath, "utf8"))
      : undefined
  );
  assert.equal(ownerClaim.batch_id, closeBatchId);
  assert.equal(ownerClaim.operation, "project.close");
  assert.equal(ownerClaim.owner_process_id, launchedA.application.pid);
  const durableOwnerClaim = JSON.parse(await readFile(lostClose.ownerPath, "utf8"));
  assert.equal(durableOwnerClaim.project_root, closeProject);
  assert.equal(durableOwnerClaim.app_instance, ownerClaim.app_instance);
  assert.equal(durableOwnerClaim.process_id, ownerClaim.owner_process_id);
  assert.equal(durableOwnerClaim.generation, ownerClaim.generation);
  assert.equal(typeof durableOwnerClaim.claim_id, "string");
  assert(durableOwnerClaim.claim_id.length > 0);
  assert.equal(
    await pathExists(closeClaimedOwnerTracePath)
      ? parseNdjson(await readFile(closeClaimedOwnerTracePath, "utf8")).length
      : 0,
    0,
    "the claimed owner delivered the envelope before its SIGKILL boundary",
  );
  const killedClaimedOwner = await killPackaged(launchedA);
  launchedA = undefined;
  assert.equal(killedClaimedOwner.browserPid, ownerClaim.owner_process_id);

  launchedA = await launchPackaged(
    xvfb.display,
    closeConfig,
    null,
    {
      OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: closeRestartTracePath,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: "project.close",
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: closeTakeoverMarkerPath,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: closeTakeoverReleasePath,
    },
  );
  const takeoverClaim = await waitFor("replacement close owner takeover", async () =>
    await pathExists(closeTakeoverMarkerPath)
      ? JSON.parse(await readFile(closeTakeoverMarkerPath, "utf8"))
      : undefined
  );
  assert.equal(takeoverClaim.batch_id, closeBatchId);
  assert.equal(takeoverClaim.operation, "project.close");
  assert.equal(takeoverClaim.owner_process_id, launchedA.application.pid);
  assert.notEqual(takeoverClaim.owner_process_id, ownerClaim.owner_process_id);
  assert(takeoverClaim.generation > ownerClaim.generation);
  const durableTakeoverClaim = JSON.parse(await readFile(lostClose.ownerPath, "utf8"));
  assert.equal(durableTakeoverClaim.app_instance, takeoverClaim.app_instance);
  assert.equal(durableTakeoverClaim.process_id, takeoverClaim.owner_process_id);
  assert.equal(durableTakeoverClaim.generation, takeoverClaim.generation);
  assert.notEqual(durableTakeoverClaim.claim_id, durableOwnerClaim.claim_id);
  launchedB = await launchPackaged(xvfb.display, closeConfig, null, {
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: closeContenderTracePath,
  });
  await waitFor("concurrent replacement owner rejection", () =>
    launchedB.stderr().includes("owned by live app") ? true : undefined
  );
  assert.equal(
    await launchedB.client.evaluate(
      'window.omegat.rpc("sys.version", {}).then((value) => value.version)',
      true,
    ),
    "6.2.0",
  );
  assert.equal(
    await pathExists(closeContenderTracePath)
      ? parseNdjson(await readFile(closeContenderTracePath, "utf8")).length
      : 0,
    0,
    "the rejected replacement delivered an envelope owned by another process",
  );
  await writeFile(closeTakeoverReleasePath, "release\n", "utf8");
  await waitFor("detached close, team, save, and refresh FIFO drained", async () =>
    !await pathExists(lostClose.activePath)
      && !await pathExists(lostClose.refreshJournalPath)
      ? true
      : undefined
  );
  const detachedClosed = await workspaceState(launchedA.client);
  assert.equal(detachedClosed.project, null);
  assert.equal(detachedClosed.welcome, true);
  assert.equal(detachedClosed.activeSurfaces, 0);
  assert.equal(detachedClosed.key, null);
  assert.equal(
    await launchedA.client.evaluate(
      'window.omegat.rpc("sys.version", {}).then((value) => value.version)',
      true,
    ),
    "6.2.0",
  );
  const closeRestartTrace = parseNdjson(
    await readFile(closeRestartTracePath, "utf8"),
  );
  assertOrderedDispatch(
    closeRestartTrace,
    [closeBatchId, closeTeamBatchId, closeSaveBatchId, closeTailBatchId],
    "detached lost close acknowledgement restart",
  );
  assert.equal(
    closeRestartTrace.some((row) =>
      row.batch_id === "lost-close-initial-entry"
    ),
    false,
    "restart replayed the acknowledged entry receipt before close",
  );
  assert.equal(
    closeRestartTrace.filter((row) => row.batch_id === closeBatchId).length,
    1,
    "restart dispatched the close receipt more than once",
  );
  assert.deepEqual(
    await snapshotStableProjectTree(closeProject),
    stableTreeBeforeRecovery,
    "close receipt recovery replayed TMX or another stable project-tree write",
  );
  const closeHistory = parseNdjson(
    await readFile(lostClose.teamHistoryPath, "utf8"),
  );
  assert.equal(
    closeHistory.filter((row) =>
      row.batch_id === closeBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "lost close acknowledgement produced more than one terminal history row",
  );
  assert.equal(
    closeHistory.filter((row) =>
      row.batch_id === closeTeamBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "close team tail produced more than one terminal history row",
  );
  assert.equal(
    closeHistory.filter((row) =>
      row.batch_id === closeSaveBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "close save tail produced more than one terminal history row",
  );
  const closeRefreshHistory = parseNdjson(
    await readFile(lostClose.refreshHistoryPath, "utf8"),
  );
  assert.equal(
    closeRefreshHistory.filter((row) =>
      row.batch_id === closeTailBatchId && row.status === "completed"
    ).length,
    1,
    "close refresh tail produced more than one terminal history row",
  );
  assert.equal(await readFile(lostClose.remotePath, "utf8"), closeRemoteBeforeRecovery);
  assert.equal(
    (await stat(lostClose.remotePath, { bigint: true })).mtimeNs,
    closeRemoteMtimeBeforeRecovery,
    "owner takeover replayed the committed team tail",
  );
  await terminatePackaged(launchedA);
  launchedA = undefined;
  await terminatePackaged(launchedB);
  launchedB = undefined;

  launchedA = await launchPackaged(
    xvfb.display,
    closeConfig,
    closeProject,
  );
  const reopenedClose = await workspaceState(launchedA.client);
  assert.equal(reopenedClose.project, closeProject);
  assert.equal(reopenedClose.source, lostClose.source);
  assert.equal(reopenedClose.translation, lostClose.translation);
  assert.equal(reopenedClose.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(reopenedClose.key), lostClose.key);
  const reopenedEntries = await launchedA.client.evaluate(
    'window.omegat.rpc("entry.list", {})',
    true,
  );
  assert.equal(reopenedEntries.length, 2);
  const reopenedWanted = reopenedEntries.find((entry) =>
    entry.key.file === lostClose.key.file
  );
  const reopenedDecoy = reopenedEntries.find((entry) =>
    entry.key.file === lostClose.decoyKey.file
  );
  assert.deepEqual(reopenedWanted.key, lostClose.key);
  assert.equal(reopenedWanted.translation, lostClose.translation);
  assert.deepEqual(reopenedDecoy.key, lostClose.decoyKey);
  assert.equal(reopenedDecoy.translation, "");
  closeReceiptRecovery = {
    lostAckBatchId: closeBatchId,
    teamTailBatchId: closeTeamBatchId,
    saveTailBatchId: closeSaveBatchId,
    refreshTailBatchId: closeTailBatchId,
    restartedDispatchOrder: [
      closeBatchId,
      closeTeamBatchId,
      closeSaveBatchId,
      closeTailBatchId,
    ],
    rendererStayedClosedDuringRecovery: true,
    concurrentReplacementRejectedByDurableOwner: true,
    claimedOwnerKilledBeforeRendererDelivery: {
      claimId: durableOwnerClaim.claim_id,
      killedBrowserPid: killedClaimedOwner.browserPid,
      killedSidecarPid: killedClaimedOwner.sidecarPid,
      deliveredEnvelopes: 0,
    },
    uniqueReplacementTakeover: {
      claimId: durableTakeoverClaim.claim_id,
      browserPid: takeoverClaim.owner_process_id,
      selectedBatchId: takeoverClaim.batch_id,
    },
    stableProjectTreeReplayed: false,
    teamRemoteWriteReplayed: false,
    completeEntryKey: lostClose.key,
    decoyEntryKey: lostClose.decoyKey,
    document3SurfacesAfterReopen: reopenedClose.activeSurfaces,
    selectedHeadCrash: {
      killedSidecarPid: closeSelectedMarker.sidecar_pid,
      replacementSidecarPid: replacementAfterCloseHead.pid,
      selectedBatchId: closeSelectedMarker.batch_id,
    },
    killedAfterLostAck: killedAfterLostClose,
  };
  receiptAckMatrix.push({
    receiptType: "close",
    lostAckBatchId: closeBatchId,
    notReplayed: ["lost-close-initial-entry"],
    restartedDispatchOrder: [
      closeBatchId,
      closeTeamBatchId,
      closeSaveBatchId,
      closeTailBatchId,
    ],
    trailingReceiptsDrained: true,
  });
  await terminatePackaged(launchedA);
  launchedA = undefined;

  const headConfig = join(workDir, "selected-head-crash-config");
  const headProject = join(workDir, "selected-head-crash-project");
  const headRemote = join(workDir, "selected-head-crash-remote");
  const headMarkerPath = join(workDir, "selected-head-sidecar-kill.json");
  const headTracePath = join(workDir, "selected-head-envelope-trace.ndjson");
  const headAckTracePath = join(workDir, "selected-head-ack-trace.ndjson");
  const selectedHead = await prepareMixedReceiptProject(
    headConfig,
    headProject,
    headRemote,
    "selected-head",
  );
  const headRemotePath = join(headRemote, "source", "shared.txt");
  const headRemoteBefore = await readFile(headRemotePath, "utf8");
  const headRemoteMtimeBefore = (await stat(headRemotePath, { bigint: true })).mtimeNs;
  launchedA = await launchPackaged(xvfb.display, headConfig, headProject, {
    OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_FOR: "commit-source",
    OMEGAT_TEST_KILL_SIDECAR_AFTER_TRANSACTION_HEAD_MARKER: headMarkerPath,
    OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: headTracePath,
    OMEGAT_TEST_TRANSACTION_ACK_TRACE: headAckTracePath,
  });
  const selectedMarker = await waitFor("selected-head sidecar SIGKILL", async () =>
    await pathExists(headMarkerPath)
      ? JSON.parse(await readFile(headMarkerPath, "utf8"))
      : undefined
  );
  assert.equal(selectedMarker.batch_id, selectedHead.teamBatchId);
  assert.equal(selectedMarker.operation, "commit-source");
  assert.equal(selectedMarker.signal, "SIGKILL");
  await waitFor("selected-head recovery FIFO drained", async () =>
    !await pathExists(selectedHead.activePath)
      && !await pathExists(selectedHead.refreshJournalPath)
      ? true
      : undefined
  );
  const replacementSidecar = await waitFor(
    "replacement sidecar after selected-head kill",
    async () => {
      const processes = await descendants(launchedA.application.pid);
      return processes.find(({ command, pid }) =>
        command.includes("omegat-sidecar") && pid !== selectedMarker.sidecar_pid
      );
    },
  );
  assert.notEqual(replacementSidecar.pid, selectedMarker.sidecar_pid);
  const headTrace = parseNdjson(await readFile(headTracePath, "utf8"));
  assert.equal(
    headTrace[0]?.batch_id,
    selectedHead.teamBatchId,
    "replacement sidecar skipped the head selected before SIGKILL",
  );
  assertOrderedDispatch(
    headTrace,
    [
      selectedHead.teamBatchId,
      selectedHead.refreshOneBatchId,
      selectedHead.refreshTwoBatchId,
    ],
    "selected-head sidecar recovery",
  );
  const recoveredHead = await workspaceState(launchedA.client);
  assert.equal(recoveredHead.project, headProject);
  assert.equal(recoveredHead.source, selectedHead.source);
  assert.equal(recoveredHead.activeSurfaces, 1);
  assert.deepEqual(JSON.parse(recoveredHead.key), selectedHead.key);
  assert.equal(await readFile(headRemotePath, "utf8"), headRemoteBefore);
  assert.equal(
    (await stat(headRemotePath, { bigint: true })).mtimeNs,
    headRemoteMtimeBefore,
    "sidecar head recovery replayed the selected team write",
  );
  const selectedHeadHistory = parseNdjson(
    await readFile(selectedHead.teamHistoryPath, "utf8"),
  );
  assert.equal(
    selectedHeadHistory.filter((row) =>
      row.batch_id === selectedHead.teamBatchId
      && row.status === "completed"
      && row.payload.phase === "renderer-acknowledged"
    ).length,
    1,
    "selected-head recovery duplicated the terminal team receipt",
  );
  selectedHeadCrashRecovery = {
    selectedBatchId: selectedHead.teamBatchId,
    killedSidecarPid: selectedMarker.sidecar_pid,
    replacementSidecarPid: replacementSidecar.pid,
    recoveredDispatchOrder: [
      selectedHead.teamBatchId,
      selectedHead.refreshOneBatchId,
      selectedHead.refreshTwoBatchId,
    ],
    productWriteReplayed: false,
    completeEntryKey: selectedHead.key,
  };
  await terminatePackaged(launchedA);
  launchedA = undefined;

  for (const headKind of ["close", "team", "save"]) {
    const label = `atomic-${headKind}`;
    const config = join(workDir, `${label}-config`);
    const project = join(workDir, `${label}-project`);
    const remote = join(workDir, `${label}-remote`);
    const oldOwnerMarkerPath = join(workDir, `${label}-old-owner.json`);
    const oldOwnerReleasePath = join(workDir, `${label}-old-owner-release`);
    const preKillTracePath = join(workDir, `${label}-pre-kill-trace.ndjson`);
    const electionMarkerPath = join(workDir, `${label}-winner.json`);
    const electionReleasePath = join(workDir, `${label}-winner-release`);
    const replacementTracePaths = [
      join(workDir, `${label}-replacement-a-trace.ndjson`),
      join(workDir, `${label}-replacement-b-trace.ndjson`),
    ];
    const prepared = await prepareAtomicElectionProject(
      config,
      project,
      remote,
      label,
      headKind,
    );
    const startupProject = headKind === "close" ? null : project;

    launchedA = await launchPackaged(xvfb.display, config, startupProject, {
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: prepared.operation,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: oldOwnerMarkerPath,
      OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: oldOwnerReleasePath,
    });
    const oldOwnerMarker = await waitFor(`${headKind} old owner claim`, async () =>
      await pathExists(oldOwnerMarkerPath)
        ? JSON.parse(await readFile(oldOwnerMarkerPath, "utf8"))
        : undefined
    );
    assert.equal(oldOwnerMarker.batch_id, prepared.headBatchId);
    assert.equal(oldOwnerMarker.operation, prepared.operation);
    assert.equal(oldOwnerMarker.owner_process_id, launchedA.application.pid);
    const oldDurableOwner = JSON.parse(await readFile(prepared.ownerPath, "utf8"));
    assert.equal(oldDurableOwner.process_id, launchedA.application.pid);
    assert.equal(oldDurableOwner.app_instance, oldOwnerMarker.app_instance);
    assert.equal(oldDurableOwner.generation, oldOwnerMarker.generation);
    assert.equal(typeof oldDurableOwner.claim_id, "string");
    assert(oldDurableOwner.claim_id.length > 0);

    launchedB = await launchPackaged(xvfb.display, config, null, {
      OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: preKillTracePath,
    });
    const preKillScope = {
      root: project,
      app_instance: `${label}-pre-kill-contender`,
      owner_process_id: launchedB.application.pid,
      generation: oldOwnerMarker.generation + 1,
    };
    const preKillPending = await invokeRpcResult(
      launchedB.client,
      "transaction.receipt.pending",
      preKillScope,
    );
    assert.equal(
      preKillPending.resolved,
      false,
      `${headKind} contender obtained an envelope while the old owner lived`,
    );
    assert.match(preKillPending.error, /locked by another process|owned by live app/);
    const preKillAck = await invokeRpcResult(
      launchedB.client,
      "transaction.receipt.ack",
      {
        ...preKillScope,
        batch_id: prepared.headBatchId,
        operation: prepared.operation,
        outcome: "succeeded",
      },
    );
    assert.equal(
      preKillAck.resolved,
      false,
      `${headKind} contender acknowledged the old owner's product head`,
    );
    assert.match(preKillAck.error, /locked by another process|owned by live app/);
    assert.deepEqual(
      JSON.parse(await readFile(prepared.ownerPath, "utf8")),
      oldDurableOwner,
      `${headKind} pre-kill contender changed the durable owner`,
    );
    assert.equal(
      await pathExists(preKillTracePath)
        ? parseNdjson(await readFile(preKillTracePath, "utf8")).length
        : 0,
      0,
      `${headKind} pre-kill contender received a renderer envelope`,
    );
    await terminatePackaged(launchedB);
    launchedB = undefined;

    const stableTreeBeforeElection = await snapshotStableProjectTree(project);
    const tmxPath = join(project, "omegat", "project_save.tmx");
    const tmxBeforeElection = await readFile(tmxPath);
    const tmxMtimeBeforeElection = (await stat(tmxPath, { bigint: true })).mtimeNs;
    const remoteBeforeElection = prepared.remotePath
      ? await readFile(prepared.remotePath)
      : null;
    const remoteMtimeBeforeElection = prepared.remotePath
      ? (await stat(prepared.remotePath, { bigint: true })).mtimeNs
      : null;

    const killedOldOwner = await killPackaged(launchedA);
    launchedA = undefined;
    assert.equal(killedOldOwner.browserPid, oldOwnerMarker.owner_process_id);
    assert.equal(
      await pathExists(`/proc/${killedOldOwner.browserPid}`),
      false,
      `${headKind} old owner PID remained live before replacement launch`,
    );

    [launchedA, launchedB] = await Promise.all(
      replacementTracePaths.map((tracePath) =>
        launchPackaged(xvfb.display, config, startupProject, {
          OMEGAT_TEST_TRANSACTION_ENVELOPE_TRACE: tracePath,
          OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: prepared.operation,
          OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: electionMarkerPath,
          OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: electionReleasePath,
        })
      ),
    );
    const electionMarker = await waitFor(
      `${headKind} simultaneous replacement winner`,
      async () =>
        await pathExists(electionMarkerPath)
          ? JSON.parse(await readFile(electionMarkerPath, "utf8"))
          : undefined,
    );
    assert.equal(electionMarker.batch_id, prepared.headBatchId);
    assert.equal(electionMarker.operation, prepared.operation);
    assert.notEqual(electionMarker.owner_process_id, killedOldOwner.browserPid);
    const replacements = [launchedA, launchedB];
    const winnerIndex = replacements.findIndex(
      (replacement) =>
        replacement.application.pid === electionMarker.owner_process_id,
    );
    assert.notEqual(winnerIndex, -1);
    const loserIndex = 1 - winnerIndex;
    const winner = replacements[winnerIndex];
    const loser = replacements[loserIndex];
    const durableWinner = JSON.parse(await readFile(prepared.ownerPath, "utf8"));
    assert.equal(durableWinner.process_id, winner.application.pid);
    assert.equal(durableWinner.app_instance, electionMarker.app_instance);
    assert.equal(durableWinner.generation, electionMarker.generation);
    assert.notEqual(durableWinner.claim_id, oldDurableOwner.claim_id);
    assert.equal(typeof durableWinner.claim_id, "string");
    assert(durableWinner.claim_id.length > 0);

    for (const tracePath of replacementTracePaths) {
      assert.equal(
        await pathExists(tracePath)
          ? parseNdjson(await readFile(tracePath, "utf8")).length
          : 0,
        0,
        `${headKind} replacement delivered before the winner release`,
      );
    }
    const loserScope = {
      root: project,
      app_instance: `${label}-simultaneous-loser`,
      owner_process_id: loser.application.pid,
      generation: electionMarker.generation + 1,
    };
    const loserPending = await invokeRpcResult(
      loser.client,
      "transaction.receipt.pending",
      loserScope,
    );
    assert.equal(
      loserPending.resolved,
      false,
      `${headKind} losing replacement obtained the product head`,
    );
    assert.match(loserPending.error, /locked by another process|owned by live app/);
    const loserAck = await invokeRpcResult(
      loser.client,
      "transaction.receipt.ack",
      {
        ...loserScope,
        batch_id: prepared.headBatchId,
        operation: prepared.operation,
        outcome: "succeeded",
      },
    );
    assert.equal(
      loserAck.resolved,
      false,
      `${headKind} losing replacement acknowledged the product head`,
    );
    assert.match(loserAck.error, /locked by another process|owned by live app/);
    assert.deepEqual(
      JSON.parse(await readFile(prepared.ownerPath, "utf8")),
      durableWinner,
      `${headKind} losing replacement changed the winning claim`,
    );

    await writeFile(electionReleasePath, "release\n", "utf8");
    await waitFor(`${headKind} product head and refresh tail drain`, async () =>
      !await pathExists(prepared.activePath)
        && !await pathExists(prepared.refreshJournalPath)
        ? true
        : undefined
    );
    const winnerTrace = parseNdjson(
      await readFile(replacementTracePaths[winnerIndex], "utf8"),
    );
    assertOrderedDispatch(
      winnerTrace,
      [prepared.headBatchId, prepared.refreshBatchId],
      `${headKind} simultaneous replacement recovery`,
    );
    assert.equal(
      winnerTrace.filter((row) => row.batch_id === prepared.headBatchId).length,
      1,
      `${headKind} winner received the product head more than once`,
    );
    assert.equal(
      await pathExists(replacementTracePaths[loserIndex])
        ? parseNdjson(
            await readFile(replacementTracePaths[loserIndex], "utf8"),
          ).length
        : 0,
      0,
      `${headKind} loser received a renderer envelope`,
    );

    const productHistory = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    assert.equal(
      productHistory.filter((row) =>
        row.batch_id === prepared.headBatchId
        && row.status === "completed"
        && row.payload.phase === "renderer-acknowledged"
      ).length,
      1,
      `${headKind} product head has duplicate terminal history`,
    );
    const refreshHistory = parseNdjson(
      await readFile(prepared.refreshHistoryPath, "utf8"),
    );
    assert.equal(
      refreshHistory.filter((row) =>
        row.batch_id === prepared.refreshBatchId
        && row.status === "completed"
      ).length,
      1,
      `${headKind} refresh tail has duplicate terminal history`,
    );
    assert.deepEqual(
      await snapshotStableProjectTree(project),
      stableTreeBeforeElection,
      `${headKind} election replayed a stable project write`,
    );
    assert.deepEqual(await readFile(tmxPath), tmxBeforeElection);
    assert.equal(
      (await stat(tmxPath, { bigint: true })).mtimeNs,
      tmxMtimeBeforeElection,
      `${headKind} election replayed the TMX write`,
    );
    if (prepared.remotePath) {
      assert.deepEqual(await readFile(prepared.remotePath), remoteBeforeElection);
      assert.equal(
        (await stat(prepared.remotePath, { bigint: true })).mtimeNs,
        remoteMtimeBeforeElection,
        "team election replayed the remote write",
      );
      assert.equal(
        await readFile(prepared.remotePath, "utf8"),
        prepared.remoteContent,
      );
    }

    const winnerWorkspace = await workspaceState(winner.client);
    if (headKind === "close") {
      assert.equal(winnerWorkspace.project, null);
      assert.equal(winnerWorkspace.welcome, true);
      assert.equal(winnerWorkspace.activeSurfaces, 0);
    } else {
      assert.equal(winnerWorkspace.project, project);
      assert.equal(winnerWorkspace.source, prepared.source);
      assert.equal(winnerWorkspace.translation, prepared.translation);
      assert.equal(winnerWorkspace.activeSurfaces, 1);
      assert.deepEqual(JSON.parse(winnerWorkspace.key), prepared.key);
      const entries = await winner.client.evaluate(
        'window.omegat.rpc("entry.list", {})',
        true,
      );
      const wanted = entries.find((entry) => entry.key.file === prepared.key.file);
      const decoy = entries.find((entry) => entry.key.file === prepared.decoyKey.file);
      assert.deepEqual(wanted.key, prepared.key);
      assert.equal(wanted.translation, prepared.translation);
      assert.deepEqual(decoy.key, prepared.decoyKey);
      assert.equal(decoy.translation, "");
    }

    atomicReplacementElectionResults.push({
      headKind,
      oldOwnerExitedBeforeReplacementLaunch: true,
      oldOwner: {
        browserPid: killedOldOwner.browserPid,
        sidecarPid: killedOldOwner.sidecarPid,
        claimId: oldDurableOwner.claim_id,
      },
      simultaneousReplacementCount: replacements.length,
      winner: {
        browserPid: winner.application.pid,
        claimId: durableWinner.claim_id,
        selectedBatchId: electionMarker.batch_id,
        dispatchOrder: [prepared.headBatchId, prepared.refreshBatchId],
      },
      loser: {
        browserPid: loser.application.pid,
        deliveredEnvelopes: 0,
        pendingRejected: true,
        acknowledgementRejected: true,
      },
      preKillContender: {
        pendingRejected: true,
        acknowledgementRejected: true,
        deliveredEnvelopes: 0,
      },
      terminalHeadCount: 1,
      terminalRefreshCount: 1,
      tmxWriteReplayed: false,
      teamRemoteWriteReplayed: false,
      completeEntryKey: prepared.key,
      decoyEntryKey: prepared.decoyKey,
      winnerDocument3Surfaces: winnerWorkspace.activeSurfaces,
    });

    await terminatePackaged(launchedA);
    launchedA = undefined;
    await terminatePackaged(launchedB);
    launchedB = undefined;

    if (headKind === "close") {
      launchedA = await launchPackaged(xvfb.display, config, project);
      const reopened = await workspaceState(launchedA.client);
      assert.equal(reopened.project, project);
      assert.equal(reopened.source, prepared.source);
      assert.equal(reopened.translation, prepared.translation);
      assert.equal(reopened.activeSurfaces, 1);
      assert.deepEqual(JSON.parse(reopened.key), prepared.key);
      const result = atomicReplacementElectionResults.at(-1);
      result.winnerDocument3SurfacesAfterExplicitReopen = reopened.activeSurfaces;
      await terminatePackaged(launchedA);
      launchedA = undefined;
    }
  }

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    simultaneousElectronInstances: true,
    sharedConfigDirectory: true,
    scenarios: results,
    productCompactionScenarios: productCompactionResults,
    mixedReceiptRecovery,
    receiptAckMatrix,
    closeReceiptRecovery,
    selectedHeadCrashRecovery,
    atomicReplacementElectionResults,
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
