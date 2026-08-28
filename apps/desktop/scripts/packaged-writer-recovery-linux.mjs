// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
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

async function startXvfb() {
  const child = spawn(
    "Xvfb",
    ["-displayfd", "3", "-screen", "0", "1600x1000x24", "-nolisten", "tcp"],
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

async function invokeRpcResult(client, method, params) {
  return client.evaluate(`(() => window.omegat.rpc(
    ${JSON.stringify(method)},
    ${JSON.stringify(params)}
  ).then(
    (value) => ({ resolved: true, value }),
    (error) => ({ resolved: false, error: String(error) })
  ))()`, true);
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

async function launchPackagedRenderer(display, configDir, project, extraEnv = {}) {
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
  await waitFor("renderer RPC bridge", () =>
    client.evaluate('typeof window.omegat?.rpc === "function"')
  );
  return { application, client, stderr: () => stderr };
}

async function workspaceState(client) {
  return client.evaluate(`(() => {
    const app = document.querySelector(".app");
    const active = document.querySelector(".editor-segment.is-active");
    return {
      project: app?.dataset.projectId || null,
      generation: Number(app?.dataset.projectGeneration ?? 0),
      welcome: document.querySelector(".welcome") !== null,
      source: active?.querySelector(".src")?.textContent ?? "",
      translation: active?.querySelector(".editor-surface")?.textContent ?? "",
      key: active?.getAttribute("data-entry-key") ?? null,
    };
  })()`);
}

async function launchPackaged(display, configDir, project, extraEnv = {}) {
  const launched = await launchPackagedRenderer(display, configDir, project, extraEnv);
  const workspace = await waitFor(
    project ? `workspace for ${project}` : "closed renderer workspace",
    async () => {
      const state = await workspaceState(launched.client);
      return project
        ? state.project === project && state.key ? state : undefined
        : state.project === null && state.welcome ? state : undefined;
    },
  );
  return { ...launched, workspace };
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
  await waitFor(
    "SIGKILLed sidecar",
    async () => !await pathExists(`/proc/${sidecarProcess.pid}`),
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
  try {
    await waitFor(
      "terminated packaged Electron",
      async () => !await pathExists(`/proc/${pid}`),
      5_000,
    );
  } catch {
    try {
      process.kill(-pid, "SIGKILL");
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
    await waitFor("SIGKILLed packaged Electron cleanup", async () =>
      !await pathExists(`/proc/${pid}`)
    );
  }
}

class SidecarSession {
  constructor(configDir, extraEnv = {}) {
    this.child = spawn(sidecar, [], {
      env: {
        ...process.env,
        OMEGAT_CONFIG_DIR: configDir,
        ...extraEnv,
      },
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
        if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
        else pending.resolve(message.result);
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

async function waitForSelector(client, selector) {
  await waitFor(selector, () =>
    client.evaluate(`document.querySelector(${JSON.stringify(selector)}) !== null`)
  );
}

async function click(client, selector) {
  const clicked = await client.evaluate(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!(element instanceof HTMLElement)) return false;
    element.click();
    return true;
  })()`);
  assert.equal(clicked, true, `element not found: ${selector}`);
}

async function setControl(client, selector, value) {
  const changed = await client.evaluate(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (
      !(element instanceof HTMLInputElement)
      && !(element instanceof HTMLTextAreaElement)
      && !(element instanceof HTMLSelectElement)
    ) return false;
    const prototype = element instanceof HTMLInputElement
      ? HTMLInputElement.prototype
      : element instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLSelectElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(
      element,
      ${JSON.stringify(value)},
    );
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  assert.equal(changed, true, `control not found: ${selector}`);
  await client.evaluate(
    "new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))",
    true,
  );
}

async function snapshotFile(path) {
  try {
    const info = await stat(path, { bigint: true });
    assert(info.isFile(), `${path} is not a regular file`);
    return {
      exists: true,
      bytes: (await readFile(path)).toString("base64"),
      mtimeNs: info.mtimeNs.toString(),
      size: info.size.toString(),
    };
  } catch (error) {
    if (error.code === "ENOENT") return { exists: false };
    throw error;
  }
}

async function snapshotFiles(paths) {
  return Object.fromEntries(
    await Promise.all(paths.map(async (path) => [path, await snapshotFile(path)])),
  );
}

async function assertSnapshots(expected, label) {
  for (const [path, snapshot] of Object.entries(expected)) {
    assert.deepEqual(await snapshotFile(path), snapshot, `${label}: ${path}`);
  }
}

async function assertSnapshotBytes(expected, label) {
  for (const [path, snapshot] of Object.entries(expected)) {
    const current = await snapshotFile(path);
    assert.equal(current.exists, snapshot.exists, `${label}: ${path} existence`);
    if (snapshot.exists) {
      assert.equal(current.bytes, snapshot.bytes, `${label}: ${path} bytes`);
      assert.equal(current.size, snapshot.size, `${label}: ${path} size`);
    }
  }
}

async function stableSnapshot(path, label, stableMs = 250) {
  let previous = await snapshotFile(path);
  let stableSince = Date.now();
  return waitFor(label, async () => {
    const current = await snapshotFile(path);
    if (JSON.stringify(current) !== JSON.stringify(previous)) {
      previous = current;
      stableSince = Date.now();
      return undefined;
    }
    return Date.now() - stableSince >= stableMs ? current : undefined;
  });
}

function parseNdjson(raw) {
  return raw.trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
}

async function prepareWriterProject(workDir, label, operation) {
  const config = join(workDir, `${label}-config`);
  const project = join(workDir, `${label}-project`);
  const wikiSource = join(workDir, `${label}-wiki.txt`);
  const exportPath = join(workDir, `${label}-export.tmx`);
  const session = new SidecarSession(config);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(join(project, "source", "source.txt"), `${label} writer source`, "utf8");
  await session.request("project.reload", {});
  const entries = await session.request("entry.list", {});
  assert.equal(entries.length, 1);
  await session.request("entry.set", {
    index: 0,
    key: entries[0].key,
    translation: "alpha writer translation",
    note: "",
    revision: entries[0].revision,
    default_translation: true,
  });
  await writeFile(wikiSource, `${label} imported wiki`, "utf8");
  await writeFile(exportPath, `${label} export before`, "utf8");
  const glossaryPath = join(project, "glossary", "glossary.txt");
  await writeFile(glossaryPath, `${label}\tbaseline\n`, "utf8");
  await session.request("project.reload", {});
  await session.close();

  const products = {
    "glossary.add": [glossaryPath],
    "search.replace": [join(project, "omegat", "project_save.tmx")],
    "wiki.import": [join(project, "source", `${label}-wiki.txt`)],
    "tmx.export": [exportPath],
    "script.run": [
      join(project, "omegat", "project_save.tmx"),
      glossaryPath,
    ],
  }[operation];
  assert(products);
  return {
    config,
    project,
    wikiSource,
    exportPath,
    glossaryPath,
    products,
    activePath: join(project, ".repositories", "transactions", "active.json"),
    historyPath: join(project, ".repositories", "transactions", "history.ndjson"),
  };
}

async function driveWriter(client, operation, prepared, label) {
  const actions = {
    "glossary.add": async () => {
      await click(client, '[data-operation-action="glossary"]');
      await waitForSelector(client, '[data-window-id="glossary-add"]');
      await setControl(
        client,
        '[data-setting="glossary-source"]',
        `${label} durable term`,
      );
      await setControl(
        client,
        '[data-setting="glossary-target"]',
        `${label} terme durable`,
      );
      await click(client, '[data-action="add-glossary"]');
    },
    "search.replace": async () => {
      await click(client, '[data-operation-action="replace"]');
      await waitForSelector(client, '[data-window-id="replace"]');
      await setControl(client, '[data-setting="search-query"]', "alpha");
      await setControl(client, '[data-setting="replace-text"]', "beta");
      await click(client, '[data-action="replace-all"]');
    },
    "wiki.import": async () => {
      await click(client, '[data-operation-action="wiki"]');
      await waitForSelector(client, '[data-window-id="wiki"]');
      await setControl(client, '[data-setting="wiki-source"]', prepared.wikiSource);
      await click(client, '[data-action="import-wiki"]');
    },
    "tmx.export": async () => {
      await click(client, '[data-operation-action="tmx-export"]');
      await waitForSelector(client, '[data-window-id="tmx-export"]');
      await setControl(
        client,
        '[data-setting="tmx-destination"]',
        prepared.exportPath,
      );
      await setControl(client, '[data-setting="tmx-level"]', "level2");
      await click(client, '[data-action="export-tmx"]');
    },
    "script.run": async () => {
      await click(client, '[data-operation-action="scripts"]');
      await waitForSelector(client, '[data-window-id="scripts"]');
      await setControl(
        client,
        '[data-setting="script-source"]',
        `editor.setTranslation('${label} script translation'); project.save(); glossary.addEntry('${label} script term','${label} script target','recovery');`,
      );
      await click(client, '[data-action="run-script"]');
    },
  };
  await actions[operation]();
}

async function verifyWriterResult(client, operation, prepared, label, committed) {
  const state = await waitFor(`${operation} renderer recovery`, async () => {
    const current = await workspaceState(client);
    const expected = operation === "search.replace"
      ? committed ? "beta writer translation" : "alpha writer translation"
      : operation === "script.run" && committed
        ? `${label} script translation`
        : "alpha writer translation";
    return current.translation === expected ? current : undefined;
  });
  assert(state.key, `${operation} lost its complete entry identity`);
  if (operation === "glossary.add") {
    const bytes = await readFile(prepared.glossaryPath, "utf8");
    assert.equal(
      bytes.includes(`${label} durable term\t${label} terme durable`),
      committed,
    );
  }
  if (operation === "wiki.import") {
    assert.equal(
      await pathExists(join(prepared.project, "source", `${label}-wiki.txt`)),
      committed,
    );
  }
  if (operation === "tmx.export" && committed) {
    assert.match(await readFile(prepared.exportPath, "utf8"), /<tmx version=/);
  }
  if (operation === "script.run") {
    const bytes = await readFile(prepared.glossaryPath, "utf8");
    assert.equal(bytes.includes(`${label} script term`), committed);
  }
}

async function runWriterSigkillMatrix(display, workDir) {
  const evidence = [];
  for (const operation of [
    "glossary.add",
    "search.replace",
    "wiki.import",
    "tmx.export",
    "script.run",
  ]) {
    for (const point of ["before_atomic_publish", "after_atomic_publish"]) {
      const label = `${operation.replaceAll(".", "-")}-${point.startsWith("before") ? "before" : "after"}`;
      const prepared = await prepareWriterProject(workDir, label, operation);
      const baseline = await snapshotFiles(prepared.products);
      const marker = join(workDir, `${label}.marker`);
      let launched = await launchPackaged(display, prepared.config, prepared.project, {
        OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION: operation,
        OMEGAT_TEST_PRODUCT_TRANSACTION_POINT: point,
        OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER: marker,
      });
      try {
        await driveWriter(launched.client, operation, prepared, label);
        await waitFor(`${label} durable checkpoint`, () => pathExists(marker));
        const journal = JSON.parse(await readFile(prepared.activePath, "utf8"));
        const envelope = journal.batches.find((row) =>
          row.payload?.operation === operation
        );
        assert(envelope, `${label} envelope not found`);
        assert.equal(
          envelope.status,
          point === "before_atomic_publish" ? "pending" : "sidecar_committed",
        );
        const committedSnapshot = await snapshotFiles(prepared.products);
        const killed = await killPackaged(launched);
        launched = undefined;

        launched = await launchPackaged(display, prepared.config, prepared.project);
        await waitFor(`${label} journal cleanup`, async () =>
          !await pathExists(prepared.activePath)
        );
        const committed = point === "after_atomic_publish";
        if (committed) {
          await assertSnapshots(committedSnapshot, `${label} committed replay`);
        } else {
          // Snapshot restoration necessarily rewrites a rolled-back file; only
          // committed receipt recovery must preserve its nanosecond mtime.
          await assertSnapshotBytes(baseline, `${label} rollback`);
        }
        await verifyWriterResult(
          launched.client,
          operation,
          prepared,
          label,
          committed,
        );
        const history = parseNdjson(await readFile(prepared.historyPath, "utf8"));
        const terminal = history.filter((row) =>
          row.batch_id === envelope.batch_id
          && ["completed", "cancelled"].includes(row.status)
        );
        assert.equal(terminal.length, 1, `${label} terminal history`);
        assert.equal(terminal[0].status, committed ? "completed" : "cancelled");
        evidence.push({
          operation,
          point,
          batchId: envelope.batch_id,
          killed,
          bytesStable: true,
          committedMtimeStable: committed,
        });
      } finally {
        await terminatePackaged(launched);
      }
    }
  }
  return evidence;
}

async function prepareMixedWriterFifo(workDir) {
  const config = join(workDir, "mixed-writer-config");
  const project = join(workDir, "mixed-writer-project");
  const remote = join(workDir, "mixed-writer-remote");
  const wikiSource = join(workDir, "mixed-fifo-wiki.txt");
  const exported = join(workDir, "mixed-fifo-export.tmx");
  const remoteSource = join(remote, "source", "source.txt");
  await mkdir(dirname(remoteSource), { recursive: true });
  await writeFile(remoteSource, "mixed remote before", "utf8");
  await writeFile(wikiSource, "mixed imported wiki", "utf8");
  await writeFile(exported, "mixed export before", "utf8");

  const session = new SidecarSession(config);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  const repositories = [{
    repo_type: "file",
    url: remote,
    branch: null,
    mappings: [{
      local: "/source/source.txt",
      repository: "/source/source.txt",
      includes: [],
      excludes: [],
    }],
  }];
  await session.request("team.mapping", { repositories });
  await session.request("team.sync", {});
  await writeFile(join(project, "source", "source.txt"), "mixed writer source", "utf8");
  await session.request("project.reload", {});
  const [entry] = await session.request("entry.list", {});
  assert(entry);
  await session.request("entry.set", {
    index: 0,
    key: entry.key,
    translation: "alpha mixed translation",
    note: "",
    revision: entry.revision,
    default_translation: true,
  });

  const generation = 121;
  const rows = [];
  const scoped = async (method, batchId, params = {}) => {
    const result = await session.request(method, {
      ...params,
      transaction_project_root: project,
      transaction_generation: generation,
      transaction_batch_id: batchId,
    });
    assert.equal(result.receipt?.status, "sidecar_committed", method);
    rows.push({
      batchId,
      operation: result.receipt.payload.operation,
      status: "sidecar_committed",
    });
    return result;
  };

  await scoped("glossary.add", "mixed-glossary", {
    source: "mixed term",
    target: "terme mixte",
    comment: "fifo",
  });
  await writeFile(join(project, "source", "source.txt"), "mixed remote committed", "utf8");
  await scoped("team.commit", "mixed-team", { which: "source" });
  await scoped("search.replace", "mixed-replace", {
    query: "alpha",
    replace: "beta",
    source: false,
    translation: true,
  });
  const refresh = await session.request("project.refresh.enqueue", {
    root: project,
    app_instance: "mixed-setup",
    generation,
    paths: [join(project, "source", "source.txt")],
    fingerprints: {
      [join(project, "source", "source.txt")]: "mixed-refresh-fingerprint",
    },
    sources: ["native"],
  });
  rows.push({
    batchId: refresh.batch.batch_id,
    operation: "project.external-refresh",
    status: "pending",
  });
  await scoped("wiki.import", "mixed-wiki", { source: wikiSource });
  await scoped("project.save", "mixed-save");
  await scoped("tmx.export", "mixed-export", {
    dest: exported,
    level: "level2",
  });
  await scoped("script.run", "mixed-script", {
    index: 0,
    source:
      "editor.setTranslation('mixed script final'); project.save(); glossary.addEntry('script mixed','script cible','fifo');",
  });
  await scoped("project.close", "mixed-close");
  await session.close();

  const transactions = join(project, ".repositories", "transactions");
  const activePath = join(transactions, "active.json");
  const journal = JSON.parse(await readFile(activePath, "utf8"));
  assert.deepEqual(
    journal.batches.map((row) => [row.batch_id, row.status]),
    rows.map((row) => [row.batchId, row.status]),
  );
  const products = [
    remoteSource,
    join(project, "glossary", "glossary.txt"),
    join(project, "omegat", "project_save.tmx"),
    join(project, "source", "mixed-fifo-wiki.txt"),
    exported,
  ];
  return {
    config,
    project,
    activePath,
    ownerPath: join(transactions, "renderer-owner.json"),
    historyPath: join(transactions, "history.ndjson"),
    rows,
    products,
    snapshots: await snapshotFiles(products),
  };
}

async function runMixedWriterFifo(display, workDir) {
  const prepared = await prepareMixedWriterFifo(workDir);
  const ownerMarker = join(workDir, "mixed-owner.marker");
  const ownerRelease = join(workDir, "mixed-owner.release");
  const droppedTrace = join(workDir, "mixed-dropped.ndjson");
  const recoveredTrace = join(workDir, "mixed-recovered.ndjson");
  let owner = await launchPackaged(display, prepared.config, prepared.project, {
    OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_FOR: "glossary.add",
    OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_MARKER: ownerMarker,
    OMEGAT_TEST_HOLD_AFTER_TRANSACTION_OWNER_CLAIM_RELEASE: ownerRelease,
  });
  let contender;
  let lostAckOwner;
  let recovered;
  try {
    const claim = await waitFor("mixed FIFO durable owner claim", async () =>
      await pathExists(ownerMarker)
        ? JSON.parse(await readFile(ownerMarker, "utf8"))
        : undefined
    );
    assert.equal(claim.batch_id, prepared.rows[0].batchId);
    assert.equal(claim.operation, "glossary.add");
    assert.equal(claim.owner_process_id, owner.application.pid);
    contender = await launchPackaged(display, prepared.config, null);
    const contenderPid = contender.application.pid;
    const rejected = await invokeRpcResult(
      contender.client,
      "transaction.receipt.pending",
      {
        root: prepared.project,
        app_instance: "mixed-pre-kill-contender",
        owner_process_id: contenderPid,
        generation: claim.generation,
      },
    );
    assert.equal(rejected.resolved, false);
    assert.match(rejected.error, /locked by another process|owned by live app/);
    assert.equal(await pathExists(droppedTrace), false);
    assert.equal(
      JSON.parse(await readFile(prepared.ownerPath, "utf8")).process_id,
      owner.application.pid,
    );
    await assertSnapshots(prepared.snapshots, "live owner exclusion");
    await terminatePackaged(contender);
    contender = undefined;

    const killedOwner = await killPackaged(owner);
    owner = undefined;
    lostAckOwner = await launchPackaged(
      display,
      prepared.config,
      prepared.project,
      {
        OMEGAT_TEST_DROP_TRANSACTION_ACKS_FOR: "glossary.add",
        OMEGAT_TEST_TRANSACTION_ACK_TRACE: droppedTrace,
      },
    );
    const dropped = await waitFor("lost glossary acknowledgement", async () => {
      if (!await pathExists(droppedTrace)) return undefined;
      return parseNdjson(await readFile(droppedTrace, "utf8")).find((row) =>
        row.batch_id === prepared.rows[0].batchId
        && row.operation === "glossary.add"
        && row.result === "dropped"
      );
    });
    assert(dropped);
    await assertSnapshots(prepared.snapshots, "lost acknowledgement replay");
    const killedLostAckOwner = await killPackaged(lostAckOwner);
    lostAckOwner = undefined;

    recovered = await launchPackagedRenderer(
      display,
      prepared.config,
      prepared.project,
      { OMEGAT_TEST_TRANSACTION_ACK_TRACE: recoveredTrace },
    );
    await waitFor("mixed writer FIFO drain", async () =>
      !await pathExists(prepared.activePath)
    );
    const trace = parseNdjson(await readFile(recoveredTrace, "utf8"))
      .filter((row) => row.result === "acknowledged");
    const positions = prepared.rows.map(({ batchId }) =>
      trace.findIndex((row) => row.batch_id === batchId)
    );
    assert(
      positions.every((position) => position >= 0),
      `FIFO trace omitted receipts: ${JSON.stringify(trace)}`,
    );
    assert(
      positions.every((position, index) =>
        index === 0 || positions[index - 1] < position
      ),
      `FIFO trace reordered receipts: ${JSON.stringify(trace)}`,
    );
    await assertSnapshots(prepared.snapshots, "replacement owner replay");
    const finalState = await waitFor("mixed close visible state", async () => {
      const state = await workspaceState(recovered.client);
      return state.welcome && state.project === null ? state : undefined;
    });
    assert.equal(finalState.translation, "");
    const history = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    for (const row of prepared.rows) {
      assert.equal(
        history.filter((candidate) =>
          candidate.batch_id === row.batchId
          && candidate.status === "completed"
        ).length,
        1,
        row.batchId,
      );
    }
    return {
      rejectedLiveContenderPid: contenderPid,
      killedOwner,
      killedLostAckOwner,
      fifo: prepared.rows.map(({ batchId, operation }) => ({ batchId, operation })),
      externalBytesAndMtimeStable: true,
      endedOnVisibleWelcome: true,
    };
  } finally {
    await Promise.all([
      terminatePackaged(owner),
      terminatePackaged(contender),
      terminatePackaged(lostAckOwner),
      terminatePackaged(recovered),
    ]);
  }
}

async function compileIoFaultShim(workDir) {
  const source = join(workDir, "io-fault.c");
  const library = join(workDir, "io-fault.so");
  await writeFile(
    source,
    String.raw`
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int matches(const char *kind, const char *path) {
  const char *selected = getenv("OMEGAT_TEST_IO_FAULT");
  const char *needle = getenv("OMEGAT_TEST_IO_FAULT_PATH");
  return selected && needle && strcmp(selected, kind) == 0 && path
    && strstr(path, needle) != NULL;
}

int fsync(int fd) {
  static int (*real_fsync)(int) = NULL;
  if (!real_fsync) real_fsync = dlsym(RTLD_NEXT, "fsync");
  char link_path[64];
  char target[PATH_MAX + 1];
  snprintf(link_path, sizeof(link_path), "/proc/self/fd/%d", fd);
  ssize_t length = readlink(link_path, target, PATH_MAX);
  if (length >= 0) {
    target[length] = '\0';
    if (matches("fsync", target)) {
      errno = EIO;
      return -1;
    }
  }
  return real_fsync(fd);
}

int rename(const char *old_path, const char *new_path) {
  static int (*real_rename)(const char *, const char *) = NULL;
  if (!real_rename) real_rename = dlsym(RTLD_NEXT, "rename");
  if (matches("rename", new_path)) {
    errno = EIO;
    return -1;
  }
  return real_rename(old_path, new_path);
}
`,
    "utf8",
  );
  const compiler = spawn(
    "cc",
    ["-shared", "-fPIC", "-O2", "-o", library, source, "-ldl"],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  let stderr = "";
  compiler.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const code = await new Promise((resolveExit, reject) => {
    compiler.once("error", reject);
    compiler.once("exit", resolveExit);
  });
  assert.equal(code, 0, `cannot compile I/O fault shim: ${stderr}`);
  return library;
}

async function prepareDiskFaultProject(workDir, label) {
  const config = join(workDir, `${label}-config`);
  const project = join(workDir, `${label}-project`);
  const session = new SidecarSession(config);
  await session.request("project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(
    join(project, "source", "source.txt"),
    `unrecognized${label.replaceAll("-", "")}`,
    "utf8",
  );
  await session.request("project.reload", {});
  const [entry] = await session.request("entry.list", {});
  assert(entry);
  await session.request("entry.set", {
    index: 0,
    key: entry.key,
    translation: `misspelled${label.replaceAll("-", "")}`,
    note: "",
    revision: entry.revision,
    default_translation: true,
  });
  const preferences = await session.request("prefs.get", {});
  preferences.first_time_wizard_done = true;
  preferences.script_dir = join(config, "scripts");
  await session.request("prefs.set", preferences);
  await session.close();
  return {
    config,
    project,
    prefsPath: join(config, "omegat.prefs.json"),
    wordPath: join(project, "omegat", "ignored_words.txt"),
    activePath: join(project, ".repositories", "transactions", "active.json"),
  };
}

async function runPrefsDiskFault(display, workDir, shim, kind) {
  const label = `prefs-${kind}`;
  const prepared = await prepareDiskFaultProject(workDir, label);
  const extraEnv = kind === "readonly"
    ? {}
    : {
      LD_PRELOAD: shim,
      OMEGAT_TEST_IO_FAULT: kind,
      OMEGAT_TEST_IO_FAULT_PATH: "omegat.prefs.json",
    };
  let launched = await launchPackaged(
    display,
    prepared.config,
    prepared.project,
    extraEnv,
  );
  try {
    await click(launched.client, '.topbar button[aria-label]');
    await waitForSelector(launched.client, '[data-window-id="prefs"]');
    const before = await stableSnapshot(
      prepared.prefsPath,
      `${label} settled preferences baseline`,
    );
    if (kind === "readonly") await chmod(prepared.config, 0o555);
    await setControl(
      launched.client,
      '[data-window-id="prefs"] [data-setting="locale"]',
      "fr",
    );
    await click(
      launched.client,
      '[data-window-id="prefs"] [data-action="save-preferences"]',
    );
    await waitForSelector(
      launched.client,
      '[data-window-id="prefs"] [data-persistence-error="prefs"]',
    );
    if (kind === "readonly") await chmod(prepared.config, 0o755);
    assert.deepEqual(await snapshotFile(prepared.prefsPath), before);
    await killPackaged(launched);
    launched = undefined;

    launched = await launchPackaged(display, prepared.config, prepared.project);
    await click(launched.client, '.topbar button[aria-label]');
    await waitForSelector(launched.client, '[data-window-id="prefs"]');
    assert.equal(
      await launched.client.evaluate(
        'document.querySelector(\'[data-window-id="prefs"] [data-setting="locale"]\')?.value',
      ),
      "en",
    );
    const recovered = await snapshotFile(prepared.prefsPath);
    assert.equal(recovered.exists, before.exists);
    assert.equal(recovered.bytes, before.bytes);
    assert.equal(recovered.size, before.size);
    return {
      scope: "prefs",
      fault: kind,
      faultBoundaryBytesAndMtimeStable: true,
      recoveredBytesStable: true,
    };
  } finally {
    if (kind === "readonly") await chmod(prepared.config, 0o755).catch(() => {});
    await terminatePackaged(launched);
  }
}

async function runSpellDiskFault(display, workDir, shim, kind) {
  const label = `spell-${kind}`;
  const prepared = await prepareDiskFaultProject(workDir, label);
  const before = await snapshotFile(prepared.wordPath);
  const extraEnv = kind === "readonly"
    ? {}
    : {
      LD_PRELOAD: shim,
      OMEGAT_TEST_IO_FAULT: kind,
      OMEGAT_TEST_IO_FAULT_PATH: "ignored_words.txt",
    };
  let launched = await launchPackaged(
    display,
    prepared.config,
    prepared.project,
    extraEnv,
  );
  try {
    if (kind === "readonly") {
      await chmod(join(prepared.project, "omegat"), 0o555);
    }
    await click(launched.client, '[data-operation-action="issues"]');
    await waitForSelector(
      launched.client,
      '[data-window-id="issues"] [data-action="spell-ignore"]',
    );
    await click(
      launched.client,
      '[data-window-id="issues"] [data-action="spell-ignore"]',
    );
    await waitForSelector(
      launched.client,
      '[data-window-id="issues"] [data-persistence-error="spell"]',
    );
    if (kind === "readonly") {
      await chmod(join(prepared.project, "omegat"), 0o755);
    }
    assert.deepEqual(await snapshotFile(prepared.wordPath), before);
    await killPackaged(launched);
    launched = undefined;

    launched = await launchPackaged(display, prepared.config, prepared.project);
    await waitFor("spell recovery journal cleanup", async () =>
      !await pathExists(prepared.activePath)
    );
    await click(launched.client, '[data-operation-action="issues"]');
    await waitForSelector(
      launched.client,
      '[data-window-id="issues"] [data-action="spell-ignore"]',
    );
    assert.deepEqual(await snapshotFile(prepared.wordPath), before);
    return { scope: "spell", fault: kind, bytesAndMtimeStable: true };
  } finally {
    if (kind === "readonly") {
      await chmod(join(prepared.project, "omegat"), 0o755).catch(() => {});
    }
    await terminatePackaged(launched);
  }
}

async function runDiskFaultMatrix(display, workDir) {
  const shim = await compileIoFaultShim(workDir);
  const evidence = [];
  for (const kind of ["readonly", "rename", "fsync"]) {
    evidence.push(await runPrefsDiskFault(display, workDir, shim, kind));
    evidence.push(await runSpellDiskFault(display, workDir, shim, kind));
  }
  return evidence;
}

async function prepareSharedConfigProjects(workDir, label) {
  const config = join(workDir, `${label}-config`);
  const firstProject = join(workDir, `${label}-first-project`);
  const secondProject = join(workDir, `${label}-second-project`);
  const session = new SidecarSession(config);
  for (const [project, target, source] of [
    [firstProject, "fr", `${label} first source`],
    [secondProject, "de", `${label} second source`],
  ]) {
    await session.request("project.create", {
      root: project,
      source_lang: "en",
      target_lang: target,
      sentence_seg: false,
    });
    await writeFile(join(project, "source", "source.txt"), source, "utf8");
    await session.request("project.reload", {});
  }
  await session.close();
  return {
    config,
    firstProject,
    secondProject,
    prefsPath: join(config, "omegat.prefs.json"),
    activePath: join(config, "transactions", "shared-config", "active.json"),
    historyPath: join(config, "transactions", "shared-config", "history.ndjson"),
  };
}

async function openPrefsPage(client, page) {
  await click(client, ".topbar button[aria-label]");
  await waitForSelector(client, '[data-window-id="prefs"]');
  await click(client, `[data-window-id="prefs"] [data-pref-page="${page}"]`);
}

async function savePrefsDraft(client) {
  await click(client, '[data-window-id="prefs"] [data-action="save-preferences"]');
}

function configFaultEnv(operation, point, marker) {
  return {
    OMEGAT_TEST_CONFIG_TRANSACTION_OPERATION: operation,
    OMEGAT_TEST_CONFIG_TRANSACTION_POINT: point,
    OMEGAT_TEST_CONFIG_TRANSACTION_MARKER: marker,
  };
}

async function assertProjectJournalsIsolated(projects, label) {
  for (const project of projects) {
    assert.equal(
      await pathExists(join(project, ".repositories", "transactions", "active.json")),
      false,
      `${label}: config write entered project active journal`,
    );
    assert.equal(
      await pathExists(join(project, ".repositories", "transactions", "history.ndjson")),
      false,
      `${label}: config write entered project history`,
    );
  }
}

async function runConfigOwnerDeath(display, workDir) {
  const prepared = await prepareSharedConfigProjects(workDir, "config-owner-death");
  const marker = join(workDir, "config-owner-death.marker");
  let owner;
  let contender;
  try {
    [owner, contender] = await Promise.all([
      launchPackaged(
        display,
        prepared.config,
        prepared.firstProject,
        configFaultEnv("prefs.patch", "after_enqueue", marker),
      ),
      launchPackaged(display, prepared.config, prepared.secondProject),
    ]);
    await openPrefsPage(owner.client, "general");
    await setControl(
      owner.client,
      '[data-window-id="prefs"] [data-setting="locale"]',
      "fr",
    );
    await savePrefsDraft(owner.client);
    const claim = await waitFor("shared config owner checkpoint", async () =>
      await pathExists(marker)
        ? JSON.parse(await readFile(marker, "utf8"))
        : undefined
    );
    assert.equal(claim.operation, "prefs.patch");
    assert.equal(claim.owner_process_id, owner.application.pid);
    const pending = JSON.parse(await readFile(prepared.activePath, "utf8"));
    assert.equal(pending.batches.length, 1);
    assert.equal(pending.batches[0].payload.locale, "fr");

    const killedOwner = await killPackaged(owner);
    owner = undefined;
    await openPrefsPage(contender.client, "fonts");
    await setControl(
      contender.client,
      '[data-window-id="prefs"] .prefs-grid > .form label input',
      "Shared Config Font",
    );
    await savePrefsDraft(contender.client);
    const mergeStarted = Date.now();
    const merged = await waitFor("owner-death field merge", async () => {
      const prefs = JSON.parse(await readFile(prepared.prefsPath, "utf8"));
      if (prefs.locale === "fr" && prefs.font_ui === "Shared Config Font") {
        return prefs;
      }
      if (Date.now() - mergeStarted >= 3_000) {
        throw new Error(JSON.stringify({
          prefs: {
            locale: prefs.locale,
            font_ui: prefs.font_ui,
          },
          active: await pathExists(prepared.activePath)
            ? JSON.parse(await readFile(prepared.activePath, "utf8"))
            : null,
          history: await pathExists(prepared.historyPath)
            ? parseNdjson(await readFile(prepared.historyPath, "utf8"))
            : [],
          contenderProcessAlive: await pathExists(
            `/proc/${contender.application.pid}`,
          ),
        }));
      }
      return prefs.locale === "fr" && prefs.font_ui === "Shared Config Font"
        ? prefs
        : undefined;
    }, 10_000);
    assert.equal(merged.locale, "fr");
    assert.equal(merged.font_ui, "Shared Config Font");
    await waitFor("owner-death config queue cleanup", async () =>
      !await pathExists(prepared.activePath)
    );
    await assertProjectJournalsIsolated(
      [prepared.firstProject, prepared.secondProject],
      "owner death",
    );
    const history = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    assert.deepEqual(
      history.map((row) => row.payload.locale ?? row.payload.font_ui),
      ["fr", "Shared Config Font"],
    );
    return {
      killedOwner,
      ownerBatch: claim.batch_id,
      mergedFields: { locale: merged.locale, font_ui: merged.font_ui },
      projectJournalsIsolated: true,
    };
  } finally {
    await Promise.all([terminatePackaged(owner), terminatePackaged(contender)]);
  }
}

async function runConfigLostAck(display, workDir) {
  const prepared = await prepareSharedConfigProjects(workDir, "config-lost-ack");
  const marker = join(workDir, "config-lost-ack.marker");
  let owner;
  let contender;
  try {
    [owner, contender] = await Promise.all([
      launchPackaged(
        display,
        prepared.config,
        prepared.firstProject,
        configFaultEnv("prefs.patch", "after_history_append", marker),
      ),
      launchPackaged(display, prepared.config, prepared.secondProject),
    ]);
    await openPrefsPage(owner.client, "segmentation");
    await setControl(
      owner.client,
      '[data-window-id="prefs"] .prefs-grid > .form label input',
      "lost-ack-rules.srx",
    );
    await savePrefsDraft(owner.client);
    const lostAck = await waitFor("shared config lost acknowledgement", async () =>
      await pathExists(marker)
        ? JSON.parse(await readFile(marker, "utf8"))
        : undefined
    );
    const historyAtKill = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    assert.equal(
      historyAtKill.filter((row) => row.batch_id === lostAck.batch_id).length,
      1,
    );
    assert.equal(historyAtKill.at(-1).status, "completed");
    const killedLostAckOwner = await killPackaged(owner);
    owner = undefined;

    await openPrefsPage(contender.client, "filters");
    await waitForSelector(
      contender.client,
      '[data-window-id="prefs"] .prefs-grid > .form .hit input',
    );
    await setControl(
      contender.client,
      '[data-window-id="prefs"] .prefs-grid > .form .hit input',
      "lost-ack-filter",
    );
    await savePrefsDraft(contender.client);
    const merged = await waitFor("lost-ack field merge", async () => {
      const prefs = JSON.parse(await readFile(prepared.prefsPath, "utf8"));
      const filterValue = Object.values(prefs.filter_options ?? {})
        .flatMap((options) => Object.values(options ?? {}))
        .find((value) => value === "lost-ack-filter");
      return prefs.srx_path === "lost-ack-rules.srx" && filterValue
        ? prefs
        : undefined;
    });
    await waitFor("lost-ack config queue cleanup", async () =>
      !await pathExists(prepared.activePath)
    );
    const finalHistory = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    assert.equal(
      finalHistory.filter((row) => row.batch_id === lostAck.batch_id).length,
      1,
    );
    assert.equal(finalHistory.length, 2);
    await assertProjectJournalsIsolated(
      [prepared.firstProject, prepared.secondProject],
      "lost acknowledgement",
    );
    return {
      killedLostAckOwner,
      batchId: lostAck.batch_id,
      exactlyOnceHistory: true,
      mergedSegmentationAndFilter: {
        srx_path: merged.srx_path,
        filter: "lost-ack-filter",
      },
      projectJournalsIsolated: true,
    };
  } finally {
    await Promise.all([terminatePackaged(owner), terminatePackaged(contender)]);
  }
}

async function runConfigConcurrentWriters(display, workDir) {
  const prepared = await prepareSharedConfigProjects(workDir, "config-concurrent");
  let first;
  let second;
  try {
    [first, second] = await Promise.all([
      launchPackaged(display, prepared.config, prepared.firstProject),
      launchPackaged(display, prepared.config, prepared.secondProject),
    ]);
    const [aligner, spell] = await Promise.all([
      invokeRpcResult(first.client, "aligner.configure", {
        persist: true,
        algo: "forward-backward",
        calculator: "poisson",
        source_lang: "en-US",
        target_lang: "fr-FR",
      }),
      invokeRpcResult(second.client, "spell.install", { lang: "en" }),
    ]);
    assert.equal(aligner.resolved, true, aligner.error);
    assert.equal(spell.resolved, true, spell.error);
    assert.equal(aligner.value.algo, "forward-backward");
    assert.equal(spell.value.ok, true);
    const prefs = JSON.parse(await readFile(prepared.prefsPath, "utf8"));
    assert.equal(prefs.aligner_algorithm, "forward-backward");
    assert.equal(prefs.aligner_calculator, "poisson");
    assert.equal(prefs.aligner_source_lang, "en-US");
    assert.equal(prefs.aligner_target_lang, "fr-FR");
    assert.equal(
      await pathExists(join(prepared.config, "spell", "hunspell", "en.aff")),
      true,
    );
    assert.equal(
      await pathExists(join(prepared.config, "spell", "hunspell", "en.dic")),
      true,
    );
    await assertProjectJournalsIsolated(
      [prepared.firstProject, prepared.secondProject],
      "concurrent aligner and spell",
    );
    const history = parseNdjson(await readFile(prepared.historyPath, "utf8"));
    assert.deepEqual(
      [...history.map((row) => row.operation)].sort(),
      ["aligner.configure", "spell.install"],
    );
    return {
      operations: history.map((row) => row.operation),
      alignerAndSpellSerialized: true,
      projectJournalsIsolated: true,
    };
  } finally {
    await Promise.all([terminatePackaged(first), terminatePackaged(second)]);
  }
}

async function runPreferenceTerminationBoundary(display, workDir, point) {
  const label = `config-durable-${point}`;
  const prepared = await prepareSharedConfigProjects(workDir, label);
  const marker = join(workDir, `${label}.marker`);
  let owner = await launchPackaged(display, prepared.config, prepared.firstProject, {
    OMEGAT_TEST_DURABLE_FILE_NAME: "omegat.prefs.json",
    OMEGAT_TEST_DURABLE_FILE_POINT: point,
    OMEGAT_TEST_DURABLE_FILE_MARKER: marker,
  });
  let recovery;
  try {
    await openPrefsPage(owner.client, "general");
    await setControl(
      owner.client,
      '[data-window-id="prefs"] [data-setting="locale"]',
      "fr",
    );
    await savePrefsDraft(owner.client);
    await waitFor(`${point} preference checkpoint`, () => pathExists(marker));
    const beforeKillCandidates = (await readdir(prepared.config))
      .filter((name) =>
        name.startsWith(".omegat.prefs.json.") && name.endsWith(".tmp")
      );
    if (point === "after_candidate_fsync") {
      assert.equal(beforeKillCandidates.length, 1);
    } else {
      assert.equal(beforeKillCandidates.length, 0);
    }
    const killed = await killPackaged(owner);
    owner = undefined;
    recovery = await launchPackaged(
      display,
      prepared.config,
      prepared.secondProject,
    );
    const recovered = await waitFor(`${point} preference recovery`, async () => {
      const prefs = JSON.parse(await readFile(prepared.prefsPath, "utf8"));
      return prefs.locale === "fr" ? prefs : undefined;
    });
    assert.equal(recovered.locale, "fr");
    await waitFor(`${point} config journal cleanup`, async () =>
      !await pathExists(prepared.activePath)
    );
    const residualCandidates = (await readdir(prepared.config))
      .filter((name) =>
        name.startsWith(".omegat.prefs.json.") && name.endsWith(".tmp")
      );
    assert.deepEqual(residualCandidates, []);
    await assertProjectJournalsIsolated(
      [prepared.firstProject, prepared.secondProject],
      point,
    );
    return {
      point,
      killed,
      candidateBeforeRecovery: beforeKillCandidates.length,
      residualCandidates: residualCandidates.length,
      recoveredValue: recovered.locale,
    };
  } finally {
    await Promise.all([terminatePackaged(owner), terminatePackaged(recovery)]);
  }
}

async function runSpellTerminationBoundary(display, workDir, point) {
  const label = `spell-staging-${point}`;
  const prepared = await prepareSharedConfigProjects(workDir, label);
  const marker = join(workDir, `${label}.marker`);
  const spellDir = join(prepared.config, "spell", "hunspell");
  let owner = await launchPackaged(display, prepared.config, prepared.firstProject, {
    OMEGAT_TEST_SPELL_INSTALL_LANG: "en",
    OMEGAT_TEST_SPELL_INSTALL_POINT: point,
    OMEGAT_TEST_SPELL_INSTALL_MARKER: marker,
  });
  let recovery;
  try {
    const started = await owner.client.evaluate(
      'window.omegat.rpc("spell.install", { lang: "en" }); true',
    );
    assert.equal(started, true);
    await waitFor(`${point} spell staging checkpoint`, () => pathExists(marker));
    const stagingBeforeKill = (await readdir(spellDir))
      .filter((name) => name.startsWith(".en.") && name.endsWith(".staging"));
    assert.equal(stagingBeforeKill.length, 1);
    const killed = await killPackaged(owner);
    owner = undefined;
    recovery = await launchPackaged(
      display,
      prepared.config,
      prepared.secondProject,
    );
    await waitFor(`${point} dictionary recovery`, async () =>
      await pathExists(join(spellDir, "en.aff"))
      && await pathExists(join(spellDir, "en.dic"))
      && !await pathExists(prepared.activePath)
    );
    const residualStaging = (await readdir(spellDir))
      .filter((name) => name.startsWith(".en.") && name.endsWith(".staging"));
    assert.deepEqual(residualStaging, []);
    await assertProjectJournalsIsolated(
      [prepared.firstProject, prepared.secondProject],
      point,
    );
    return {
      point,
      killed,
      stagingBeforeRecovery: stagingBeforeKill.length,
      residualStaging: residualStaging.length,
      dictionaryPairComplete: true,
    };
  } finally {
    await Promise.all([terminatePackaged(owner), terminatePackaged(recovery)]);
  }
}

async function runSharedConfigTransactionMatrix(display, workDir) {
  const ownerDeath = await runConfigOwnerDeath(display, workDir);
  const lostAck = await runConfigLostAck(display, workDir);
  const concurrent = await runConfigConcurrentWriters(display, workDir);
  const durableBoundaries = [];
  for (const point of [
    "after_candidate_fsync",
    "after_rename",
    "after_parent_fsync",
  ]) {
    durableBoundaries.push(
      await runPreferenceTerminationBoundary(display, workDir, point),
    );
  }
  const spellStaging = [];
  for (const point of [
    "after_staging_fsync",
    "after_aff_rename",
    "after_parent_fsync",
  ]) {
    spellStaging.push(
      await runSpellTerminationBoundary(display, workDir, point),
    );
  }
  return {
    ownerDeath,
    lostAck,
    concurrent,
    durableBoundaries,
    spellStaging,
  };
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises visible writer recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-writer-recovery-e2e-"));
const xvfb = await startXvfb();
try {
  const scope = process.env.OMEGAT_WRITER_E2E_SCOPE ?? "all";
  const sigkill = scope === "shared-config"
    ? []
    : await runWriterSigkillMatrix(xvfb.display, workDir);
  const fifo = scope === "shared-config"
    ? null
    : await runMixedWriterFifo(xvfb.display, workDir);
  const diskFaults = scope === "shared-config"
    ? []
    : await runDiskFaultMatrix(xvfb.display, workDir);
  const sharedConfig = scope === "legacy"
    ? null
    : await runSharedConfigTransactionMatrix(xvfb.display, workDir);
  const leftovers = await readdir(workDir);
  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    visibleUi: [
      "glossary",
      "replace",
      "wiki",
      "TMX export",
      "scripts",
      "preferences",
      "spell issues",
      "dual-project shared preferences",
      "file filters",
      "segmentation",
    ],
    sigkill,
    fifo,
    diskFaults,
    sharedConfig,
    temporaryScenarioCount: leftovers.length,
    platformsNotRun: ["windows", "macos"],
  }));
} finally {
  try {
    process.kill(xvfb.child.pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  await rm(workDir, { recursive: true, force: true });
}
