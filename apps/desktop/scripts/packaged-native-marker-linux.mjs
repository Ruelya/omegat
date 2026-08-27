// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
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
  assert.equal(code, 0, `sidecar seed failed: ${stderr}`);
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

if (process.platform !== "linux") {
  throw new Error("This E2E exercises a real Linux package");
}
await Promise.all([access(executable), access(sidecar), access(pluginLibrary)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-native-marker-e2e-"));
const configDir = join(workDir, "config");
const pluginDir = join(configDir, "plugins", "example");
const projectDir = join(workDir, "project");
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
await writeFile(
  join(projectDir, "source", "marker.example"),
  "Hello from plugin\n",
  "utf8",
);
await rpcOnce(configDir, "project.create", {
  root: projectDir,
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
        OMEGAT_PROJECT: projectDir,
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
      surface?.focus();
      return Boolean(surface && window.omegat?.rpc);
    })()`);
    return ready || undefined;
  });
  await client.command("Input.insertText", { text: "😀 plugin" });

  const rendered = await waitFor("renderer native Marker tooltip", async () => {
    const state = await client.evaluate(`(() => {
      const mark = document.querySelector(".product-marker-native-plugin");
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
    title: "Example marker in source/marker.example",
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
    /^plugin marker example\.native-marker failed: isolated worker exited /,
  );

  const alive = await client.evaluate("window.omegat.rpc('sys.version', {})", true);
  assert.equal(alive.version, "6.2.0");
  const rendererAlive = await client.evaluate(`({
    marked: document.querySelector(".product-marker-native-plugin")?.textContent ?? null,
    title: document.querySelector(".product-marker-native-plugin")?.getAttribute("title") ?? null
  })`);
  assert.deepEqual(rendererAlive, {
    marked: "plugin",
    title: "Example marker in source/marker.example",
  });

  console.log(JSON.stringify({
    result: "passed",
    package: executable,
    plugin: "example.native-marker",
    markedText: rendered.marked,
    tooltip: rendered.title,
    crashRejected: crash.rejected,
    sidecarAfterCrash: alive.version,
    rendererAfterCrash: rendererAlive.marked,
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
