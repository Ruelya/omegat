// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { execFile as execFileCallback, spawn } from "node:child_process";
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
import { promisify } from "node:util";

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
const execFile = promisify(execFileCallback);

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
  const target = await waitFor("packaged renderer", () => pageTarget(port));
  const client = new DevToolsClient(target.webSocketDebuggerUrl);
  await client.connect();
  await client.command("Runtime.enable");
  await waitFor("project workspace", async () => {
    const state = await client.evaluate(`(() => ({
      project: document.querySelector(".app")?.dataset.projectId ?? null,
      key: document.querySelector(".editor-segment.is-active")
        ?.getAttribute("data-entry-key") ?? null,
    }))()`);
    return state.project === project && state.key ? state : undefined;
  });
  return { application, client, stderr: () => stderr };
}

async function killPackaged(launched) {
  const processes = await descendants(launched.application.pid);
  const sidecarProcess = processes.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  assert(sidecarProcess, `packaged sidecar not found: ${JSON.stringify(processes)}`);
  const browserPid = launched.application.pid;
  const exited = new Promise((resolveExit) =>
    launched.application.once("exit", resolveExit)
  );
  process.kill(-browserPid, "SIGKILL");
  await Promise.race([exited, sleep(5_000)]);
  await waitFor("SIGKILLed Electron", () =>
    pathExists(`/proc/${browserPid}`).then((value) => !value)
  );
  await waitFor("SIGKILLed sidecar", () =>
    pathExists(`/proc/${sidecarProcess.pid}`).then((value) => !value)
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
  await waitFor("terminated packaged Electron", () =>
    pathExists(`/proc/${pid}`).then((value) => !value)
  );
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

async function git(args, cwd) {
  const result = await execFile("git", args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  return result.stdout.trim();
}

async function bareGitState(remote) {
  const head = await git([
    "--git-dir",
    remote,
    "rev-parse",
    "refs/heads/main",
  ]);
  const names = (await git([
    "--git-dir",
    remote,
    "ls-tree",
    "-r",
    "--name-only",
    "refs/heads/main",
  ])).split(/\r?\n/).filter(Boolean);
  const files = [];
  for (const name of names) {
    const { stdout } = await execFile(
      "git",
      ["--git-dir", remote, "show", `refs/heads/main:${name}`],
      { encoding: null, maxBuffer: 16 * 1024 * 1024 },
    );
    files.push([name, Buffer.from(stdout).toString("base64")]);
  }
  return { head, files };
}

async function findGitWorktree(project) {
  const repositories = join(project, ".repositories");
  for (const entry of await readdir(repositories, { withFileTypes: true })) {
    if (
      entry.isDirectory()
      && await pathExists(join(repositories, entry.name, ".git"))
    ) {
      return join(repositories, entry.name);
    }
  }
  throw new Error("packaged Git worktree was not created");
}

async function assertGitCoherent(project, remote, expected) {
  const state = await bareGitState(remote);
  assert.deepEqual(state.files, expected.files);
  const worktree = await findGitWorktree(project);
  assert.equal(await git(["rev-parse", "HEAD"], worktree), state.head);
  assert.equal(await git(["status", "--porcelain"], worktree), "");
  assert.deepEqual(
    await snapshot(worktree, { ignoreTopLevel: [".git"] }),
    expected.files,
  );
  return state;
}

function parseNdjson(raw) {
  return raw.trim().split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
}

function faultEnv(operation, point, marker) {
  return {
    OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION: operation,
    OMEGAT_TEST_PRODUCT_TRANSACTION_POINT: point,
    OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER: marker,
  };
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
      activeSurfaces: document.querySelectorAll(
        ".editor-segment.is-active .editor-surface"
      ).length,
    };
  })()`);
}

async function placeActiveCaret(client) {
  assert.equal(
    await client.evaluate(`(() => {
      const surface = document.querySelector(".editor-segment.is-active .editor-surface");
      surface?.focus();
      return document.activeElement?.classList.contains("ime-proxy") ?? false;
    })()`),
    true,
    "packaged editor did not focus its native input proxy",
  );
  for (let index = 0; index < 5; index += 1) {
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
  return editorState(client);
}

async function assertEditorIntegrity(client, wantedKey, translation) {
  const state = await editorState(client);
  const parsedKey = JSON.parse(state.key);
  assert.deepEqual(
    Object.keys(parsedKey).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
  );
  assert.deepEqual(parsedKey, wantedKey);
  assert.equal(state.translation, translation);
  assert.equal(state.activeSurfaces, 1);
  assert(state.caret >= 0 && state.caret <= translation.length);

  const entries = await client.evaluate('window.omegat.rpc("entry.list", {})', true);
  const wanted = entries.find((entry) =>
    JSON.stringify(entry.key) === JSON.stringify(wantedKey)
  );
  const decoy = entries.find((entry) =>
    entry.key.source_text === wantedKey.source_text
    && JSON.stringify(entry.key) !== JSON.stringify(wantedKey)
  );
  assert(wanted, "wanted complete-key entry disappeared");
  assert(decoy, "same-source decoy entry disappeared");
  assert.equal(wanted.translation, translation);
  assert.equal(decoy.translation, "");
  return state;
}

async function openTeamWindow(client) {
  await client.evaluate(`(() => {
    const button = document.querySelector('[data-operation-action="team-window"]');
    button?.click();
  })()`);
  await waitFor("visible Team window", async () =>
    await client.evaluate(
      `Boolean(document.querySelector('[data-operation-action="team-sync"]'))`,
    )
      ? true
      : undefined
  );
}

async function triggerVisibleTeamOperation(client, action) {
  await openTeamWindow(client);
  assert.equal(
    await client.evaluate(`(() => {
      const button = document.querySelector(
        '[data-operation-action=${JSON.stringify(action)}]'
      );
      if (!(button instanceof HTMLButtonElement) || button.disabled) return false;
      button.click();
      return true;
    })()`),
    true,
    `visible ${action} action was unavailable`,
  );
}

async function activeEnvelope(path, status) {
  return waitFor(`${status} team transaction envelope`, async () => {
    if (!await pathExists(path)) return undefined;
    const envelope = JSON.parse(await readFile(path, "utf8"));
    return envelope.status === status ? envelope : undefined;
  });
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises packaged multi-repository team recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-team-rename-e2e-"));
const configDir = join(workDir, "config");
const project = join(workDir, "project");
const mainRemote = join(workDir, "main-git-remote.git");
const mainSeed = join(workDir, "main-git-seed");
const mappingRemote = join(workDir, "mapping-file-remote");
const active = join(project, ".repositories", "transactions", "active.json");
const history = join(project, ".repositories", "transactions", "history.ndjson");
const syncBeforeMarker = join(workDir, "team-sync-before.marker");
const syncAfterMarker = join(workDir, "team-sync-after.marker");
const commitBeforeMarker = join(workDir, "team-commit-before.marker");
const commitAfterMarker = join(workDir, "team-commit-after.marker");
const duplicateSource = "PACKAGED MULTI REPOSITORY DUPLICATE SOURCE";
const wantedTranslation = "WANTED TEAM TRANSLATION 😀 TAIL";
const mainProduct = join(project, "team-main.marker");
const mappingProduct = join(project, "team-mapping.marker");
const repositories = [
  {
    repo_type: "git",
    url: mainRemote,
    branch: "main",
    mappings: [{
      local: "/team-main.marker",
      repository: "/main.marker",
      includes: [],
      excludes: [],
    }],
  },
  {
    repo_type: "file",
    url: mappingRemote,
    branch: null,
    mappings: [{
      local: "/team-mapping.marker",
      repository: "/mapping.marker",
      includes: [],
      excludes: [],
    }],
  },
];
const xvfb = await startXvfb();
let launched;

try {
  await Promise.all([
    mkdir(configDir, { recursive: true }),
    mkdir(mainSeed, { recursive: true }),
    mkdir(mappingRemote, { recursive: true }),
  ]);
  await git(["init", "--bare", mainRemote], workDir);
  await git(["init", "-b", "main"], mainSeed);
  await git(["config", "user.name", "OmegaT E2E"], mainSeed);
  await git(["config", "user.email", "omegat-e2e@example.invalid"], mainSeed);
  await writeFile(join(mainSeed, "main-seed.keep"), "main seed\n", "utf8");
  await git(["add", "-A"], mainSeed);
  await git(["commit", "-m", "seed packaged team Git remote"], mainSeed);
  await git(["remote", "add", "origin", mainRemote], mainSeed);
  await git(["push", "-u", "origin", "HEAD:refs/heads/main"], mainSeed);
  await rpcOnce(configDir, "project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await Promise.all([
    writeFile(join(project, "source", "a-wanted.txt"), duplicateSource, "utf8"),
    writeFile(join(project, "source", "b-decoy.txt"), duplicateSource, "utf8"),
    writeFile(mainProduct, "main-v1\n", "utf8"),
    writeFile(mappingProduct, "mapping-v1\n", "utf8"),
    writeFile(join(mappingRemote, "mapping-seed.keep"), "mapping seed\n", "utf8"),
  ]);

  launched = await launchPackaged(xvfb.display, configDir, project);
  const setup = await launched.client.evaluate(`(async () => {
    const entries = await window.omegat.rpc("entry.list", {});
    const wanted = entries.find((entry) => entry.key.file === "a-wanted.txt");
    const decoy = entries.find((entry) => entry.key.file === "b-decoy.txt");
    if (!wanted || !decoy) throw new Error("duplicate setup entries missing");
    await window.omegat.rpc("entry.set", {
      index: wanted.index,
      key: wanted.key,
      translation: ${JSON.stringify(wantedTranslation)},
      note: "packaged team receipt",
      revision: wanted.revision,
      default_translation: false,
    });
    await window.omegat.rpc("project.save", {});
    const mapping = await window.omegat.rpc(
      "team.mapping",
      ${JSON.stringify({ repositories })}
    );
    return { wanted: wanted.key, decoy: decoy.key, mapping };
  })()`, true);
  assert.equal(setup.mapping.ok, true);
  assert.equal(setup.mapping.repositories.length, 2);
  assert.equal(setup.wanted.source_text, setup.decoy.source_text);
  assert.notDeepEqual(setup.wanted, setup.decoy);
  assert.deepEqual(
    Object.keys(setup.wanted).sort(),
    ["file", "id", "next", "path", "prev", "source_text"],
  );
  await terminatePackaged(launched);
  launched = undefined;

  // team.sync: kill while the product, Git remote, and file remote have
  // changed but before the sidecar receipt crosses active.json's atomic rename.
  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("team.sync", "before_atomic_publish", syncBeforeMarker),
  );
  await assertEditorIntegrity(launched.client, setup.wanted, wantedTranslation);
  const syncBeforeEditor = await placeActiveCaret(launched.client);
  const syncProjectBefore = await snapshot(project, {
    ignoreTopLevel: [".repositories"],
  });
  const syncGitBefore = await bareGitState(mainRemote);
  const syncMappingBefore = await snapshot(mappingRemote);
  await triggerVisibleTeamOperation(launched.client, "team-sync");
  await waitFor("team.sync pre-rename checkpoint", () => pathExists(syncBeforeMarker));
  const syncPending = await activeEnvelope(active, "pending");
  assert.equal(syncPending.payload.operation, "sync");
  assert.deepEqual(await editorState(launched.client), syncBeforeEditor);
  assert.notDeepEqual(await bareGitState(mainRemote), syncGitBefore);
  assert.notDeepEqual(await snapshot(mappingRemote), syncMappingBefore);
  const syncKilledBefore = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  const syncRecoveredEditor = await assertEditorIntegrity(
    launched.client,
    setup.wanted,
    wantedTranslation,
  );
  await waitFor("team.sync pending recovery cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  assert.deepEqual(
    await snapshot(project, { ignoreTopLevel: [".repositories"] }),
    syncProjectBefore,
  );
  await assertGitCoherent(project, mainRemote, syncGitBefore);
  assert.deepEqual(await snapshot(mappingRemote), syncMappingBefore);
  await terminatePackaged(launched);
  launched = undefined;

  // team.sync: kill after the receipt rename while the renderer is still
  // waiting for the blocked sidecar response. Recovery must only clean state.
  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("team.sync", "after_atomic_publish", syncAfterMarker),
  );
  await assertEditorIntegrity(launched.client, setup.wanted, wantedTranslation);
  const syncAfterEditor = await placeActiveCaret(launched.client);
  await triggerVisibleTeamOperation(launched.client, "team-sync");
  await waitFor("team.sync post-rename checkpoint", () => pathExists(syncAfterMarker));
  const syncCommitted = await activeEnvelope(active, "sidecar_committed");
  assert.equal(syncCommitted.payload.operation, "sync");
  assert.equal(syncCommitted.commit.manifest_sha256.length, 64);
  assert.deepEqual(await editorState(launched.client), syncAfterEditor);
  const syncCommittedProject = await snapshot(project, {
    ignoreTopLevel: [".repositories"],
  });
  const syncCommittedGit = await bareGitState(mainRemote);
  const syncCommittedMapping = await snapshot(mappingRemote);
  const syncKilledAfter = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  const syncCommittedEditor = await assertEditorIntegrity(
    launched.client,
    setup.wanted,
    wantedTranslation,
  );
  await waitFor("team.sync committed receipt cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  const syncGeneration = await launched.client.evaluate(
    'Number(document.querySelector(".app")?.dataset.projectGeneration ?? 0)',
  );
  const duplicateSyncAck = await launched.client.evaluate(
    `window.omegat.acknowledgeTeamReceipt(
      ${JSON.stringify(project)},
      ${JSON.stringify(syncGeneration)},
      ${JSON.stringify(syncCommitted.batch_id)}
    )`,
    true,
  );
  assert.equal(duplicateSyncAck.ack.acknowledged, true);
  assert.equal(duplicateSyncAck.ack.already_acknowledged, true);
  assert.deepEqual(
    await snapshot(project, { ignoreTopLevel: [".repositories"] }),
    syncCommittedProject,
  );
  assert.deepEqual(await bareGitState(mainRemote), syncCommittedGit);
  await assertGitCoherent(project, mainRemote, syncCommittedGit);
  assert.deepEqual(await snapshot(mappingRemote), syncCommittedMapping);
  await terminatePackaged(launched);
  launched = undefined;

  // Give team.commit a new project product while leaving both remotes at v1.
  await Promise.all([
    writeFile(mainProduct, "main-v2\n", "utf8"),
    writeFile(mappingProduct, "mapping-v2\n", "utf8"),
  ]);

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("team.commit", "before_atomic_publish", commitBeforeMarker),
  );
  await assertEditorIntegrity(launched.client, setup.wanted, wantedTranslation);
  const commitBeforeEditor = await placeActiveCaret(launched.client);
  const commitProjectBefore = await snapshot(project, {
    ignoreTopLevel: [".repositories"],
  });
  const commitGitBefore = await bareGitState(mainRemote);
  const commitMappingBefore = await snapshot(mappingRemote);
  await triggerVisibleTeamOperation(launched.client, "team-commit-source");
  await waitFor("team.commit pre-rename checkpoint", () => pathExists(commitBeforeMarker));
  const commitPending = await activeEnvelope(active, "pending");
  assert.equal(commitPending.payload.operation, "commit-source");
  assert.deepEqual(await editorState(launched.client), commitBeforeEditor);
  assert.notDeepEqual(await bareGitState(mainRemote), commitGitBefore);
  assert.notDeepEqual(await snapshot(mappingRemote), commitMappingBefore);
  const commitKilledBefore = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  const commitRecoveredEditor = await assertEditorIntegrity(
    launched.client,
    setup.wanted,
    wantedTranslation,
  );
  await waitFor("team.commit pending recovery cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  assert.deepEqual(
    await snapshot(project, { ignoreTopLevel: [".repositories"] }),
    commitProjectBefore,
  );
  await assertGitCoherent(project, mainRemote, commitGitBefore);
  assert.deepEqual(await snapshot(mappingRemote), commitMappingBefore);
  await terminatePackaged(launched);
  launched = undefined;

  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv("team.commit", "after_atomic_publish", commitAfterMarker),
  );
  await assertEditorIntegrity(launched.client, setup.wanted, wantedTranslation);
  const commitAfterEditor = await placeActiveCaret(launched.client);
  await triggerVisibleTeamOperation(launched.client, "team-commit-source");
  await waitFor("team.commit post-rename checkpoint", () => pathExists(commitAfterMarker));
  const commitCommitted = await activeEnvelope(active, "sidecar_committed");
  assert.equal(commitCommitted.payload.operation, "commit-source");
  assert.equal(commitCommitted.commit.manifest_sha256.length, 64);
  assert.deepEqual(await editorState(launched.client), commitAfterEditor);
  const commitProduct = await snapshot(project, {
    ignoreTopLevel: [".repositories"],
  });
  const committedGit = await bareGitState(mainRemote);
  const committedMapping = await snapshot(mappingRemote);
  const commitKilledAfter = await killPackaged(launched);
  launched = undefined;

  launched = await launchPackaged(xvfb.display, configDir, project);
  const commitCommittedEditor = await assertEditorIntegrity(
    launched.client,
    setup.wanted,
    wantedTranslation,
  );
  await waitFor("team.commit committed receipt cleanup", async () =>
    await pathExists(active) ? undefined : true
  );
  const commitGeneration = await launched.client.evaluate(
    'Number(document.querySelector(".app")?.dataset.projectGeneration ?? 0)',
  );
  const duplicateCommitAck = await launched.client.evaluate(
    `window.omegat.acknowledgeTeamReceipt(
      ${JSON.stringify(project)},
      ${JSON.stringify(commitGeneration)},
      ${JSON.stringify(commitCommitted.batch_id)}
    )`,
    true,
  );
  assert.equal(duplicateCommitAck.ack.acknowledged, true);
  assert.equal(duplicateCommitAck.ack.already_acknowledged, true);
  assert.deepEqual(
    await snapshot(project, { ignoreTopLevel: [".repositories"] }),
    commitProduct,
  );
  assert.deepEqual(await bareGitState(mainRemote), committedGit);
  await assertGitCoherent(project, mainRemote, committedGit);
  assert.deepEqual(await snapshot(mappingRemote), committedMapping);
  assert.equal(
    await git(["--git-dir", mainRemote, "show", "refs/heads/main:main.marker"]),
    "main-v2",
  );
  assert.equal(
    await readFile(join(mappingRemote, "mapping.marker"), "utf8"),
    "mapping-v2\n",
  );

  const rows = parseNdjson(await readFile(history, "utf8"));
  assert.equal(
    rows.filter((row) =>
      row.batch_id === syncPending.batch_id
      && row.payload.operation === "sync"
      && row.status === "cancelled"
    ).length,
    1,
  );
  assert.equal(
    rows.filter((row) =>
      row.batch_id === syncCommitted.batch_id
      && row.payload.operation === "sync"
      && row.status === "completed"
    ).length,
    1,
    "post-receipt team.sync was replayed",
  );
  assert.equal(
    rows.filter((row) =>
      row.batch_id === commitPending.batch_id
      && row.payload.operation === "commit-source"
      && row.status === "cancelled"
    ).length,
    1,
  );
  assert.equal(
    rows.filter((row) =>
      row.batch_id === commitCommitted.batch_id
      && row.payload.operation === "commit-source"
      && row.status === "completed"
    ).length,
    1,
    "post-receipt team.commit was replayed",
  );

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    repositories: repositories.map(({ repo_type, url, mappings }) => ({
      repo_type,
      url,
      mappings,
    })),
    completeEntryKey: setup.wanted,
    singleDocument3: commitCommittedEditor.activeSurfaces,
    teamSync: {
      beforeRename: {
        batchId: syncPending.batch_id,
        killed: syncKilledBefore,
        projectRollback: true,
        remoteRollback: { gitHeadAndWorktree: true, fileMapping: true },
        editorBeforeKill: syncBeforeEditor,
        editorAfterRecovery: syncRecoveredEditor,
      },
      afterRenameBeforeRendererAck: {
        batchId: syncCommitted.batch_id,
        killed: syncKilledAfter,
        receiptPreserved: true,
        rendererAckRecovered: true,
        duplicateAckIdempotent: duplicateSyncAck.ack.already_acknowledged,
        replayedWrites: 0,
        editorBeforeKill: syncAfterEditor,
        editorAfterRecovery: syncCommittedEditor,
      },
    },
    teamCommit: {
      beforeRename: {
        batchId: commitPending.batch_id,
        killed: commitKilledBefore,
        projectRollback: true,
        remoteRollback: { gitHeadAndWorktree: true, fileMapping: true },
        editorBeforeKill: commitBeforeEditor,
        editorAfterRecovery: commitRecoveredEditor,
      },
      afterRenameBeforeRendererAck: {
        batchId: commitCommitted.batch_id,
        killed: commitKilledAfter,
        receiptPreserved: true,
        rendererAckRecovered: true,
        duplicateAckIdempotent: duplicateCommitAck.ack.already_acknowledged,
        replayedWrites: 0,
        editorBeforeKill: commitAfterEditor,
        editorAfterRecovery: commitCommittedEditor,
      },
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
    process.stderr.write(`Retained packaged E2E work directory: ${workDir}\n`);
  } else {
    await rm(workDir, { recursive: true, force: true });
  }
}
