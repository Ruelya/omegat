// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const WAIT_MS = 30_000;
const desktopDir = resolve(import.meta.dirname, "..");
const executable = join(desktopDir, "release", "linux-unpacked", "omegat-desktop");
const releaseDir = resolve(desktopDir, "..", "..", "target", "release");
const sidecar = join(releaseDir, "omegat-sidecar");
const pluginLibrary = join(releaseDir, "libomegat_example_plugin.so");

const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

async function waitFor(label, check) {
  const deadline = Date.now() + WAIT_MS;
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
  const port = address.port;
  await new Promise((resolveClose, reject) =>
    server.close((error) => error ? reject(error) : resolveClose()),
  );
  return port;
}

async function rpcBatch(configDir, requests) {
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
  child.stdin.end(requests.map(({ method, params }, index) =>
    JSON.stringify({ jsonrpc: "2.0", id: index + 1, method, params })
  ).join("\n") + "\n");
  const code = await new Promise((resolveExit, reject) => {
    child.once("error", reject);
    child.once("exit", resolveExit);
  });
  assert.equal(code, 0, `sidecar seed failed: ${stderr}`);
  const responses = stdout.trim().split(/\r?\n/).map((line) => JSON.parse(line));
  assert.equal(responses.length, requests.length);
  for (const response of responses) {
    assert.equal(response.error, undefined, JSON.stringify(response.error));
  }
  return responses.map(({ result }) => result);
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
  throw new Error("This E2E exercises a real Linux package");
}
await Promise.all([access(executable), access(sidecar), access(pluginLibrary)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-native-marker-e2e-"));
const configDir = join(workDir, "config");
const pluginDir = join(configDir, "plugins", "example");
const projectDir = join(workDir, "project");
const markerTrace = join(workDir, "marker-trace.log");
await Promise.all([
  mkdir(pluginDir, { recursive: true }),
  mkdir(join(projectDir, "source"), { recursive: true }),
]);
await copyFile(pluginLibrary, join(pluginDir, "libomegat_example_plugin.so"));
await writeFile(
  join(pluginDir, "omegat-plugin.toml"),
  [
    'id = "example"',
    'name = "Example native plugin"',
    'version = "1.0.0"',
    'plugin_type = "filter"',
    'entry = "libomegat_example_plugin.so"',
    "",
  ].join("\n"),
);
const sourceLines = [
  "Hello from plugin",
  "Slow marker edge",
  ...Array.from({ length: 24 }, (_, index) =>
    `Segment ${String(index + 3).padStart(2, "0")}`
  ),
];
await writeFile(
  join(projectDir, "source", "marker.example"),
  `${sourceLines.join("\n")}\n`,
  "utf8",
);
await rpcBatch(configDir, [
  {
    method: "project.create",
    params: {
      root: projectDir,
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: false,
    },
  },
  {
    method: "entry.set",
    params: {
      index: 1,
      translation: "slow plugin",
      note: "",
      revision: 1,
      default_translation: true,
    },
  },
  { method: "project.save", params: {} },
]);

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
        OMEGAT_PROJECT: projectDir,
        OMEGAT_EXAMPLE_MARKER_DELAY_MS: "2500",
        OMEGAT_EXAMPLE_MARKER_TRACE: markerTrace,
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

  const listed = await waitFor("packaged native Marker registration", async () => {
    const markers = await client.evaluate("window.omegat?.rpc('markers.list', {})", true);
    return markers?.some((marker) => marker.id === "example.native-marker")
      ? markers
      : undefined;
  });
  assert.deepEqual(listed, [{
    plugin_id: "example",
    id: "example.native-marker",
    name: "org.omegat.example.NativePluginMarker",
  }]);

  await waitFor("active editor surface", async () => {
    const ready = await client.evaluate(`(() => {
      document.querySelectorAll(".modal-bg").forEach((modal) => modal.click());
      const surface = document.querySelector(".editor-surface");
      return Boolean(surface && window.omegat?.rpc);
    })()`);
    return ready || undefined;
  });

  const traceCounts = async () => {
    const trace = await readFile(markerTrace, "utf8");
    const lines = trace.split(/\r?\n/);
    return {
      starts: lines.filter((line) => line === "start\tSlow marker edge").length,
      finishes: lines.filter((line) => line === "finish\tSlow marker edge").length,
    };
  };
  await waitFor("slow inactive native Marker worker start", async () => {
    const counts = await traceCounts();
    return counts.starts > 0 ? counts : undefined;
  });

  const activateEntry = async (entryNumber) => {
    const clicked = await client.evaluate(`(() => {
      const segment = document.querySelector(
        '.editor-segment[data-entry="${entryNumber}"]'
      );
      if (!segment) return false;
      segment.click();
      return true;
    })()`);
    assert.equal(clicked, true, `entry ${entryNumber} is not in the rendered page`);
    return waitFor(`renderer entry ${entryNumber} activation`, async () => {
      const active = await client.evaluate(
        "document.querySelector('.editor-segment.is-active')?.dataset.entry ?? null",
      );
      return active === String(entryNumber) ? active : undefined;
    });
  };

  await activateEntry(9);
  await activateEntry(17);
  await sleep(250);
  const staleWorkers = await traceCounts();
  assert(staleWorkers.starts > 0);
  await waitFor("old-page native Marker workers finish", async () => {
    const counts = await traceCounts();
    return counts.finishes >= staleWorkers.starts ? counts : undefined;
  });

  await activateEntry(9);
  const staleWriteback = await client.evaluate(`(() => {
    const segment = document.querySelector('.editor-segment[data-entry="2"]');
    const target = segment?.querySelector('[data-entry-part="TRANSLATION"]');
    return {
      present: Boolean(segment),
      text: target?.textContent ?? null,
      marked: target?.querySelector(".product-marker-native-plugin")?.textContent ?? null,
    };
  })()`);
  assert.deepEqual(staleWriteback, {
    present: true,
    text: "slow plugin",
    marked: null,
  });
  await waitFor("fresh inactive native Marker worker start", async () => {
    const counts = await traceCounts();
    return counts.starts > staleWorkers.starts ? counts : undefined;
  });
  const freshInactive = await waitFor("fresh inactive Marker publication", async () => {
    const state = await client.evaluate(`(() => {
      const target = document.querySelector(
        '.editor-segment[data-entry="2"] [data-entry-part="TRANSLATION"]'
      );
      const mark = target?.querySelector(".product-marker-native-plugin");
      return {
        text: target?.textContent ?? null,
        marked: mark?.textContent ?? null,
        title: mark?.getAttribute("title") ?? null,
      };
    })()`);
    return state.marked === "plugin" ? state : undefined;
  });
  assert.deepEqual(freshInactive, {
    text: "slow plugin",
    marked: "plugin",
    title: "Example marker in marker.example",
  });

  await activateEntry(1);
  await client.evaluate(`(() => {
    const surface = document.querySelector(".editor-surface");
    surface?.focus();
    return document.activeElement?.classList.contains("ime-proxy") ?? false;
  })()`);
  await client.command("Input.insertText", { text: "😀 plugin" });

  const rendered = await waitFor("renderer native Marker tooltip", async () => {
    const state = await client.evaluate(`(() => {
      const mark = document.querySelector(
        ".editor-surface .product-marker-native-plugin"
      );
      return {
        text: document.querySelector(".editor-surface")?.textContent ?? null,
        marked: mark?.textContent ?? null,
        title: mark?.getAttribute("title") ?? null,
      };
    })()`);
    return state.marked === "plugin" ? state : undefined;
  });
  assert.deepEqual(rendered, {
    text: "😀 plugin",
    marked: "plugin",
    title: "Example marker in marker.example",
  });

  const crash = await client.evaluate(`window.omegat.rpc("markers.query", {
    id: "example.native-marker",
    entry_key: {
      file: "source/marker.example",
      source_text: "Hello from plugin",
      id: "0",
      prev: "",
      next: "",
      path: null
    },
    source_text: "Hello from plugin",
    translation_text: "plugin",
    is_active: true,
    crash_worker: true
  }).then(
    () => ({ rejected: false }),
    (error) => ({ rejected: true, message: String(error.message ?? error) })
  )`, true);
  assert.equal(crash.rejected, true);
  assert.match(
    crash.message,
    /^Error invoking remote method 'rpc': Error: plugin marker example\.native-marker failed: isolated worker exited /,
  );

  const alive = await client.evaluate("window.omegat.rpc('sys.version', {})", true);
  assert.equal(alive.version, "6.2.0");
  const rendererAlive = await client.evaluate(`({
    marked: document.querySelector(
      ".editor-surface .product-marker-native-plugin"
    )?.textContent ?? null,
    title: document.querySelector(
      ".editor-surface .product-marker-native-plugin"
    )?.getAttribute("title") ?? null
  })`);
  assert.deepEqual(rendererAlive, {
    marked: "plugin",
    title: "Example marker in marker.example",
  });

  await client.command("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "a",
    code: "KeyA",
    modifiers: 2,
    windowsVirtualKeyCode: 65,
  });
  await client.command("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "a",
    code: "KeyA",
    modifiers: 2,
    windowsVirtualKeyCode: 65,
  });
  const reloadedTranslation = "😀 plugin caret-tail";
  await client.command("Input.insertText", { text: reloadedTranslation });
  await waitFor("replacement translation", async () => {
    const text = await client.evaluate(
      "document.querySelector('.editor-surface')?.textContent ?? null",
    );
    return text === reloadedTranslation ? text : undefined;
  });
  for (let index = 0; index < 4; index += 1) {
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
  const editorState = () => client.evaluate(`(() => {
    const segment = document.querySelector(".editor-segment.is-active");
    const surface = segment?.querySelector(".editor-surface");
    const caret = surface?.querySelector(":scope > .caret");
    const following = caret
      ? [...surface.children]
          .slice([...surface.children].indexOf(caret) + 1)
          .find((child) => child.hasAttribute("data-offset"))
      : null;
    return {
      entry: Number(segment?.dataset.entry ?? 0),
      key: segment?.dataset.entryKey ?? null,
      text: surface?.textContent ?? null,
      caret: following
        ? Number(following.getAttribute("data-offset"))
        : (surface?.textContent.length ?? -1),
    };
  })()`);
  const beforeReload = await editorState();
  assert.deepEqual(beforeReload, {
    entry: 1,
    key: beforeReload.key,
    text: reloadedTranslation,
    caret: reloadedTranslation.length - 4,
  });
  assert(beforeReload.key);

  await writeFile(
    join(projectDir, "source", "a-before.example"),
    "Before reload\n",
    "utf8",
  );
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
  const afterReload = await waitFor("EntryKey-bound renderer reload", async () => {
    const state = await editorState();
    return state.entry === 2
      && state.key === beforeReload.key
      && state.text === reloadedTranslation
      && state.caret === beforeReload.caret
      ? state
      : undefined;
  });
  assert.deepEqual(afterReload, {
    entry: 2,
    key: beforeReload.key,
    text: reloadedTranslation,
    caret: beforeReload.caret,
  });
  const persistedAfterReload = await client.evaluate(
    `window.omegat.rpc("entry.get", { index: ${afterReload.entry - 1} })`,
    true,
  );
  assert.deepEqual({
    key: persistedAfterReload.key,
    translation: persistedAfterReload.translation,
  }, {
    key: JSON.parse(beforeReload.key),
    translation: reloadedTranslation,
  });

  const prefsOpened = await client.evaluate(`(() => {
    const button = document.querySelector('button[aria-label="Preferences"]');
    button?.click();
    return Boolean(button);
  })()`);
  assert.equal(prefsOpened, true);
  await waitFor("Editing preferences page", async () => {
    const opened = await client.evaluate(`(() => {
      const row = [...document.querySelectorAll(".prefs-grid nav .row")]
        .find((candidate) => candidate.textContent?.trim() === "Editing");
      row?.click();
      return Boolean(row);
    })()`);
    return opened || undefined;
  });
  const filterToggled = await waitFor("untranslated filter checkbox", async () => {
    const toggled = await client.evaluate(`(() => {
      const label = [...document.querySelectorAll(".prefs-grid .form label")]
        .find((candidate) => candidate.textContent?.includes("Untranslated only"));
      const checkbox = label?.querySelector('input[type="checkbox"]');
      if (!checkbox) return false;
      if (!checkbox.checked) checkbox.click();
      return checkbox.checked;
    })()`);
    return toggled || undefined;
  });
  assert.equal(filterToggled, true);
  await client.evaluate(`document.querySelector(".prefs-grid .form button.primary")?.click()`);
  await waitFor("persisted untranslated renderer filter", async () => {
    const prefs = await client.evaluate("window.omegat.rpc('prefs.get', {})", true);
    return prefs?.filter_untranslated === true ? prefs : undefined;
  });
  await client.evaluate(`(() => {
    const cancel = [...document.querySelectorAll(".modal button")]
      .find((button) => button.textContent?.trim() === "Cancel");
    cancel?.click();
  })()`);
  const filtered = await waitFor("filtered renderer page rebuild", async () => {
    const state = await client.evaluate(`(() => {
      const segments = [...document.querySelectorAll(".editor-segment")];
      const active = document.querySelector(".editor-segment.is-active");
      return {
        entries: segments.map((segment) => Number(segment.dataset.entry)),
        active: Number(active?.dataset.entry ?? 0),
        text: active?.querySelector(".editor-surface")?.textContent ?? null,
      };
    })()`);
    return state.active === 4
      && !state.entries.includes(2)
      && !state.entries.includes(3)
      ? state
      : undefined;
  });
  assert.equal(filtered.text, "");
  assert(filtered.entries.includes(1));

  const finalTrace = await traceCounts();
  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    plugin: "example.native-marker",
    staleInactiveMarkerDiscarded: staleWriteback.marked === null,
    staleMarkerWorkers: staleWorkers,
    finalMarkerWorkers: finalTrace,
    freshInactiveMarker: freshInactive.marked,
    markedText: rendered.marked,
    tooltip: rendered.title,
    crashRejected: crash.rejected,
    sidecarAfterCrash: alive.version,
    rendererAfterCrash: rendererAlive.marked,
    reload: {
      oldEntry: beforeReload.entry,
      newEntry: afterReload.entry,
      completeEntryKeyRetained: afterReload.key === beforeReload.key,
      caret: afterReload.caret,
      translation: persistedAfterReload.translation,
    },
    filter: filtered,
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
