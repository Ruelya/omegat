// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  mkdir,
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
  await waitFor("project workspace", async () =>
    await client.evaluate(
      `document.querySelector(".app")?.dataset.projectId === ${JSON.stringify(project)}`,
    )
  );
  return { application, client, stderr: () => stderr };
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
  const processIds = [
    launched.application.pid,
    ...(await descendants(launched.application.pid)).map(({ pid }) => pid),
  ];
  launched.client?.close();
  try {
    process.kill(-launched.application.pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  await waitFor(
    "terminated packaged process group",
    async () => {
      const alive = await Promise.all(
        processIds.map((pid) => pathExists(`/proc/${pid}`)),
      );
      return alive.every((value) => !value);
    },
  );
}

async function setInput(client, selector, value) {
  const changed = await client.evaluate(`(() => {
    const input = document.querySelector(${JSON.stringify(selector)});
    if (!(input instanceof HTMLInputElement)) return false;
    const setter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    setter?.call(input, ${JSON.stringify(value)});
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  assert.equal(changed, true, `input not found: ${selector}`);
  await client.evaluate(
    "new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))",
    true,
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

async function waitForSelector(client, selector) {
  await waitFor(selector, () =>
    client.evaluate(`document.querySelector(${JSON.stringify(selector)}) !== null`)
  );
}

async function closeWindow(client, windowId) {
  const closed = await client.evaluate(`(() => {
    const window = document.querySelector(
      ${JSON.stringify(`[data-window-id="${windowId}"]`)},
    );
    const background = window?.closest(".modal-bg");
    if (!(background instanceof HTMLElement)) return false;
    background.click();
    return true;
  })()`);
  assert.equal(closed, true, `window not found: ${windowId}`);
  await waitFor(`closed ${windowId}`, () =>
    client.evaluate(
      `document.querySelector(${JSON.stringify(`[data-window-id="${windowId}"]`)}) === null`,
    )
  );
}

async function openProperties(client) {
  await waitForSelector(client, '[data-operation-action="project-properties"]');
  await click(client, '[data-operation-action="project-properties"]');
  await waitForSelector(client, '[data-window-id="project-edit"]');
}

async function openMapping(client) {
  await waitForSelector(client, '[data-operation-action="team-window"]');
  await click(client, '[data-operation-action="team-window"]');
  await waitForSelector(client, '[data-window-id="team"]');
  await click(client, '[data-window-id="team"] [data-action="open-repository-mapping"]');
  await waitForSelector(client, '[data-window-id="mapping"]');
}

async function openPreferenceEditor(client, page, action, windowId) {
  await waitForSelector(client, ".topbar button[aria-label]");
  await click(client, ".topbar button[aria-label]");
  await waitForSelector(client, ".prefs-grid");
  await click(client, `[data-pref-page="${page}"]`);
  await click(client, `[data-action="${action}"]`);
  await waitForSelector(client, `[data-window-id="${windowId}"]`);
}

async function activeEnvelope(path, status, operation) {
  return waitFor(`${operation} ${status} envelope`, async () => {
    if (!await pathExists(path)) return undefined;
    const journal = JSON.parse(await readFile(path, "utf8"));
    const rows = Array.isArray(journal.batches) ? journal.batches : [journal];
    return rows.find((row) =>
      row.status === status && row.payload?.operation === operation
    );
  });
}

function faultEnv(operation, point, marker) {
  return {
    OMEGAT_TEST_PRODUCT_TRANSACTION_OPERATION: operation,
    OMEGAT_TEST_PRODUCT_TRANSACTION_POINT: point,
    OMEGAT_TEST_PRODUCT_TRANSACTION_MARKER: marker,
  };
}

async function closeAndReopen(launched, display, configDir, project, verify) {
  await waitForSelector(launched.client, '[data-operation-action="project-close"]');
  await click(launched.client, '[data-operation-action="project-close"]');
  await waitFor("visible closed workspace", () =>
    launched.client.evaluate(`(() => ({
      welcome: document.querySelector(".welcome") !== null,
      project: document.querySelector(".app")?.dataset.projectId ?? "",
    }))()`).then((state) => state.welcome && state.project === "")
  );
  await waitFor("close receipt acknowledgement", async () => !await pathExists(active));
  await killPackaged(launched);
  const reopened = await launchPackaged(display, configDir, project);
  await verify(reopened.client);
  return reopened;
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises visible config receipt recovery on Linux");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-config-receipt-e2e-"));
const configDir = join(workDir, "config");
const project = join(workDir, "project");
const remote = join(workDir, "file-remote");
const active = join(project, ".repositories", "transactions", "active.json");
const prefsPath = join(configDir, "omegat.prefs.json");
const segmentationBefore = join(workDir, "global-before.srx");
const segmentationAfter = join(workDir, "global-after.srx");
const xvfb = await startXvfb();
const evidence = [];
let launched;

async function runFault(operation, point, markerName, drive, verify) {
  const marker = join(workDir, markerName);
  launched = await launchPackaged(
    xvfb.display,
    configDir,
    project,
    faultEnv(operation, point, marker),
  );
  await drive(launched.client);
  const markerWaitStarted = Date.now();
  const markerOutcome = await waitFor(markerName, async () => {
    if (await pathExists(marker)) return { reached: true };
    const renderer = await launched.client.evaluate(`(() => {
      const app = document.querySelector(".app");
      const phase = app?.getAttribute("data-operation-phase") ?? "";
      return {
        operation: app?.getAttribute("data-operation") ?? "",
        phase,
        status: [...document.querySelectorAll(".status")]
          .map((node) => node.textContent?.trim() ?? "")
          .filter(Boolean),
        segmentationOpen:
          document.querySelector('[data-window-id="segmentation"]') !== null,
        srxPath:
          document.querySelector('[data-window-id="segmentation"] [data-setting="srx_path"]')
            ?.value ?? "",
      };
    })()`);
    if (renderer?.phase === "failed" || Date.now() - markerWaitStarted >= 2_000) {
      const preferences = await pathExists(prefsPath)
        ? JSON.parse(await readFile(prefsPath, "utf8"))
        : null;
      const journal = await pathExists(active)
        ? JSON.parse(await readFile(active, "utf8"))
        : null;
      return { reached: false, renderer, preferences, journal };
    }
    return undefined;
  });
  assert.equal(
    markerOutcome.reached,
    true,
    `${markerName} failed before checkpoint: ${JSON.stringify(markerOutcome)}`,
  );
  const status = point === "before_atomic_publish" ? "pending" : "sidecar_committed";
  const envelope = await activeEnvelope(active, status, operation);
  const killed = await killPackaged(launched);
  launched = undefined;
  launched = await launchPackaged(xvfb.display, configDir, project);
  await verify(launched.client);
  await waitFor(`${markerName} cleanup`, async () => !await pathExists(active));
  evidence.push({
    operation,
    point,
    batchId: envelope.batch_id,
    status,
    killed,
  });
  await killPackaged(launched);
  launched = undefined;
}

try {
  await mkdir(remote, { recursive: true });
  const segmentationRules = await readFile(
    resolve(desktopDir, "..", "..", "fixtures", "srx", "defaultRules.srx"),
    "utf8",
  );
  await Promise.all([
    writeFile(segmentationBefore, segmentationRules, "utf8"),
    writeFile(segmentationAfter, segmentationRules, "utf8"),
  ]);
  await rpcOnce(configDir, "project.create", {
    root: project,
    source_lang: "en",
    target_lang: "fr",
    sentence_seg: false,
  });
  await writeFile(join(project, "source", "source.txt"), "CONFIG RECEIPT SOURCE", "utf8");

  await runFault(
    "project.update",
    "before_atomic_publish",
    "properties-before.marker",
    async (client) => {
      await openProperties(client);
      await setInput(client, '[data-window-id="project-edit"] [data-setting="target_lang"]', "de");
      await click(client, '[data-action="save-project-properties"]');
    },
    async (client) => {
      await openProperties(client);
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-setting="target_lang"]\')?.value',
        ),
        "fr",
      );
    },
  );
  await runFault(
    "project.update",
    "after_atomic_publish",
    "properties-after.marker",
    async (client) => {
      await openProperties(client);
      await setInput(client, '[data-window-id="project-edit"] [data-setting="target_lang"]', "de");
      await click(client, '[data-action="save-project-properties"]');
    },
    async (client) => {
      await openProperties(client);
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-setting="target_lang"]\')?.value',
        ),
        "de",
      );
    },
  );

  await runFault(
    "team.mapping",
    "before_atomic_publish",
    "mapping-before.marker",
    async (client) => {
      await openMapping(client);
      await client.evaluate(`(() => {
        const select = document.querySelector(
          '[data-window-id="mapping"] [data-setting="repo_type"]',
        );
        const setter = Object.getOwnPropertyDescriptor(
          HTMLSelectElement.prototype,
          "value",
        )?.set;
        setter?.call(select, "file");
        select?.dispatchEvent(new Event("change", { bubbles: true }));
      })()`);
      await setInput(client, '[data-window-id="mapping"] [data-setting="url"]', remote);
      await click(client, '[data-action="save-repositories"]');
    },
    async (client) => {
      await openMapping(client);
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-setting="repo_type"]\')?.value',
        ),
        "git",
      );
    },
  );
  await runFault(
    "team.mapping",
    "after_atomic_publish",
    "mapping-after.marker",
    async (client) => {
      await openMapping(client);
      await client.evaluate(`(() => {
        const select = document.querySelector(
          '[data-window-id="mapping"] [data-setting="repo_type"]',
        );
        const setter = Object.getOwnPropertyDescriptor(
          HTMLSelectElement.prototype,
          "value",
        )?.set;
        setter?.call(select, "file");
        select?.dispatchEvent(new Event("change", { bubbles: true }));
      })()`);
      await setInput(client, '[data-window-id="mapping"] [data-setting="url"]', remote);
      await click(client, '[data-action="save-repositories"]');
    },
    async (client) => {
      await openMapping(client);
      assert.deepEqual(
        await client.evaluate(`(() => ({
          type: document.querySelector('[data-setting="repo_type"]')?.value,
          url: document.querySelector('[data-setting="url"]')?.value,
        }))()`),
        { type: "file", url: remote },
      );
    },
  );

  for (const [point, value] of [
    ["before_atomic_publish", "filter-before"],
    ["after_atomic_publish", "filter-after"],
  ]) {
    await runFault(
      "project.reload",
      point,
      `filter-${point}.marker`,
      async (client) => {
        await openPreferenceEditor(
          client,
          "filters",
          "open-filter-editor",
          "filters",
        );
        await waitForSelector(client, '[data-filter-open="text"]');
        await click(client, '[data-filter-open="text"]');
        await waitForSelector(client, '[data-filter-option="preserve_spaces"]');
        await setInput(
          client,
          '[data-filter-option="preserve_spaces"]',
          value,
        );
        await click(client, '[data-action="save-filter-options"]');
      },
      async (client) => {
        const prefs = JSON.parse(await readFile(prefsPath, "utf8"));
        assert.equal(prefs.filter_options.text.preserve_spaces, value);
        await openPreferenceEditor(
          client,
          "filters",
          "open-filter-editor",
          "filters",
        );
        await waitForSelector(client, '[data-filter-open="text"]');
        await click(client, '[data-filter-open="text"]');
        await waitForSelector(client, '[data-filter-option="preserve_spaces"]');
        assert.equal(
          await client.evaluate(
            'document.querySelector(\'[data-filter-option="preserve_spaces"]\')?.value',
          ),
          value,
        );
      },
    );
  }

  for (const [point, value] of [
    ["before_atomic_publish", segmentationBefore],
    ["after_atomic_publish", segmentationAfter],
  ]) {
    await runFault(
      "project.reload",
      point,
      `segmentation-${point}.marker`,
      async (client) => {
        await openPreferenceEditor(
          client,
          "segmentation",
          "open-segmentation-editor",
          "segmentation",
        );
        await setInput(
          client,
          '[data-window-id="segmentation"] [data-setting="srx_path"]',
          value,
        );
        await click(client, '[data-action="save-segmentation"]');
      },
      async (client) => {
        const prefs = JSON.parse(await readFile(prefsPath, "utf8"));
        assert.equal(prefs.srx_path, value);
        await openPreferenceEditor(
          client,
          "segmentation",
          "open-segmentation-editor",
          "segmentation",
        );
        assert.equal(
          await client.evaluate(
            'document.querySelector(\'[data-window-id="segmentation"] [data-setting="srx_path"]\')?.value',
          ),
          value,
        );
      },
    );
  }

  launched = await launchPackaged(xvfb.display, configDir, project);
  launched = await closeAndReopen(
    launched,
    xvfb.display,
    configDir,
    project,
    async (client) => {
      await openProperties(client);
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-setting="target_lang"]\')?.value',
        ),
        "de",
      );
      await closeWindow(client, "project-edit");
      await openMapping(client);
      assert.deepEqual(
        await client.evaluate(`(() => ({
          type: document.querySelector('[data-setting="repo_type"]')?.value,
          url: document.querySelector('[data-setting="url"]')?.value,
        }))()`),
        { type: "file", url: remote },
      );
      await closeWindow(client, "mapping");
      await closeWindow(client, "team");
      await openPreferenceEditor(
        client,
        "filters",
        "open-filter-editor",
        "filters",
      );
      await waitForSelector(client, '[data-filter-open="text"]');
      await click(client, '[data-filter-open="text"]');
      await waitForSelector(client, '[data-filter-option="preserve_spaces"]');
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-filter-option="preserve_spaces"]\')?.value',
        ),
        "filter-after",
      );
      await closeWindow(client, "filters");
      await click(client, '[data-pref-page="segmentation"]');
      await click(client, '[data-action="open-segmentation-editor"]');
      await waitForSelector(client, '[data-window-id="segmentation"]');
      assert.equal(
        await client.evaluate(
          'document.querySelector(\'[data-window-id="segmentation"] [data-setting="srx_path"]\')?.value',
        ),
          segmentationAfter,
      );
    },
  );
  await killPackaged(launched);
  launched = undefined;

  const history = (await readFile(
    join(project, ".repositories", "transactions", "history.ndjson"),
    "utf8",
  ))
    .trim()
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  for (const row of evidence) {
    const terminal = history.filter((candidate) =>
      candidate.batch_id === row.batchId
      && (
        candidate.status === "completed"
        || candidate.status === "cancelled"
      )
    );
    assert.equal(terminal.length, 1, `${row.operation}:${row.point}`);
    assert.equal(
      terminal[0].status,
      row.point === "before_atomic_publish" ? "cancelled" : "completed",
    );
  }

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    visibleUi: [
      "project properties",
      "file filters",
      "segmentation",
      "repository mapping",
      "project close/reopen",
    ],
    persistenceScopes: {
      projectJournal: join(project, ".repositories", "transactions"),
      globalPreferences: prefsPath,
      filterBeforeReceiptSurvivedProjectRollback: true,
      segmentationBeforeReceiptSurvivedProjectRollback: true,
    },
    evidence,
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
