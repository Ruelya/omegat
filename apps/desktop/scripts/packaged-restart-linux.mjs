// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

const WAIT_MS = 30_000;
const POLL_MS = 100;
const desktopDir = resolve(import.meta.dirname, "..");
const executable =
  process.env.OMEGAT_PACKAGED_EXECUTABLE ??
  join(desktopDir, "release", "linux-unpacked", "omegat-desktop");

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

async function waitFor(label, check, timeoutMs = WAIT_MS) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await check();
      if (result) return result;
    } catch (error) {
      lastError = error;
    }
    await sleep(POLL_MS);
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
  const { port } = address;
  await new Promise((resolveClose, reject) =>
    server.close((error) => (error ? reject(error) : resolveClose())),
  );
  return port;
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

async function sidecarChildren(pid) {
  try {
    const raw = await readFile(`/proc/${pid}/task/${pid}/children`, "utf8");
    const children = raw.trim() ? raw.trim().split(/\s+/).map(Number) : [];
    const sidecars = [];
    for (const childPid of children) {
      try {
        const cmdline = await readFile(`/proc/${childPid}/cmdline`);
        const [command] = cmdline.toString().split("\0");
        if (basename(command) === "omegat-sidecar") sidecars.push(childPid);
      } catch {
        // The child may exit while /proc is being inspected.
      }
    }
    return sidecars;
  } catch {
    return [];
  }
}

async function pageTarget(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(1_000),
  });
  if (!response.ok) throw new Error(`DevTools endpoint returned ${response.status}`);
  const targets = await response.json();
  return targets.find(
    (target) => target.type === "page" && target.webSocketDebuggerUrl,
  );
}

async function browserTarget(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
    signal: AbortSignal.timeout(1_000),
  });
  if (!response.ok) throw new Error(`DevTools endpoint returned ${response.status}`);
  return response.json();
}

async function devtoolsCommand(webSocketDebuggerUrl, method, params = {}) {
  return new Promise((resolveCommand, reject) => {
    const socket = new WebSocket(webSocketDebuggerUrl);
    const timeout = setTimeout(() => {
      socket.close();
      reject(new Error(`DevTools command timed out: ${method}`));
    }, 5_000);
    timeout.unref();
    socket.addEventListener("open", () => {
      socket.send(
        JSON.stringify({
          id: 1,
          method,
          params,
        }),
      );
    });
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id !== 1) return;
      clearTimeout(timeout);
      socket.close();
      if (message.error) {
        reject(new Error(message.error.message));
      } else {
        resolveCommand(message.result);
      }
    });
    socket.addEventListener("error", () => {
      clearTimeout(timeout);
      reject(new Error("DevTools WebSocket failed"));
    });
  });
}

async function evaluate(webSocketDebuggerUrl, expression, awaitPromise = false) {
  const result = await devtoolsCommand(
    webSocketDebuggerUrl,
    "Runtime.evaluate",
    { expression, awaitPromise, returnByValue: true },
  );
  if (result.exceptionDetails) {
    throw new Error(
      result.exceptionDetails.exception?.description ??
        "Renderer evaluation failed",
    );
  }
  return result.result?.value;
}

async function browserPid(webSocketDebuggerUrl) {
  const { processInfo } = await devtoolsCommand(
    webSocketDebuggerUrl,
    "SystemInfo.getProcessInfo",
  );
  const browser = processInfo.find(({ type }) => type === "browser");
  assert(browser, "DevTools did not report the packaged browser process");
  return browser.id;
}

async function processArgv(pid) {
  const raw = await readFile(`/proc/${pid}/cmdline`);
  return raw.toString().split("\0").filter(Boolean);
}

async function processExited(pid) {
  try {
    await access(`/proc/${pid}`);
    return false;
  } catch {
    return true;
  }
}

function terminate(pid, signal = "SIGTERM") {
  if (!pid) return;
  try {
    process.kill(pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises a real Linux package and requires Linux");
}
await access(executable);

const configDir = await mkdtemp(join(tmpdir(), "omegat-restart-e2e-"));
const port = await unusedPort();
const marker = `--omegat-restart-e2e=${randomUUID()}`;
const xvfb = await startXvfb();
let launched;
let initialSidecar;
let restartedSidecar;
let restartedPid;
let stderr = "";

try {
  launched = spawn(
    executable,
    [
      `--remote-debugging-port=${port}`,
      marker,
      "--disable-gpu",
      "--no-sandbox",
    ],
    {
      detached: true,
      env: {
        ...process.env,
        DISPLAY: xvfb.display,
        OMEGAT_CONFIG_DIR: configDir,
      },
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  launched.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const initialPid = launched.pid;
  assert(initialPid, "Packaged Electron process did not start");

  const firstBrowser = await waitFor("initial packaged browser", () =>
    browserTarget(port),
  );
  assert.equal(
    await browserPid(firstBrowser.webSocketDebuggerUrl),
    initialPid,
    "DevTools attached to a different Electron main process",
  );
  const firstTarget = await waitFor("initial packaged renderer", () =>
    pageTarget(port),
  );
  assert.equal(
    await evaluate(firstTarget.webSocketDebuggerUrl, "typeof window.omegat?.relaunch"),
    "function",
    "The packaged preload did not expose the relaunch product API",
  );
  [initialSidecar] = await waitFor("initial packaged sidecar", async () => {
    const children = await sidecarChildren(initialPid);
    return children.length ? children : undefined;
  });

  assert.equal(
    await evaluate(
      firstTarget.webSocketDebuggerUrl,
      'window.omegat.relaunch(); "relaunch-requested"',
    ),
    "relaunch-requested",
  );

  await waitFor("original Electron process to exit", () =>
    processExited(initialPid),
  );
  const secondBrowser = await waitFor("restarted Electron browser", async () => {
    const browser = await browserTarget(port);
    return browser.webSocketDebuggerUrl !== firstBrowser.webSocketDebuggerUrl
      ? browser
      : undefined;
  });
  restartedPid = await browserPid(secondBrowser.webSocketDebuggerUrl);
  const restartedArgv = await processArgv(restartedPid);
  assert(restartedArgv.includes(`--remote-debugging-port=${port}`));
  assert(restartedArgv.includes(marker));
  assert.notEqual(restartedPid, initialPid);

  await waitFor("original packaged sidecar to exit", () =>
    processExited(initialSidecar),
  );
  [restartedSidecar] = await waitFor("restarted packaged sidecar", async () => {
    const children = await sidecarChildren(restartedPid);
    return children.length ? children : undefined;
  });
  assert.notEqual(restartedSidecar, initialSidecar);

  const secondTarget = await waitFor("restarted packaged renderer", () =>
    pageTarget(port),
  );
  assert.equal(
    await evaluate(
      secondTarget.webSocketDebuggerUrl,
      "window.omegat.startup().then((context) => context.configDir)",
      true,
    ),
    configDir,
    "The restarted renderer did not become ready with its original environment",
  );
  await evaluate(
    secondTarget.webSocketDebuggerUrl,
    'window.omegat.quit(); "quit-requested"',
  );
  await waitFor("restarted Electron process to quit", () =>
    processExited(restartedPid),
  );

  console.log(
    JSON.stringify({
      result: "passed",
      package: executable,
      initialPid,
      restartedPid,
      initialSidecar,
      restartedSidecar,
      argumentsPreserved: true,
      rendererReadyAfterRestart: true,
    }),
  );
} catch (error) {
  if (stderr) process.stderr.write(stderr);
  throw error;
} finally {
  terminate(restartedSidecar);
  terminate(initialSidecar);
  terminate(restartedPid);
  if (launched?.pid) {
    try {
      process.kill(-launched.pid, "SIGTERM");
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  }
  terminate(xvfb.child.pid);
  await rm(configDir, { recursive: true, force: true });
}
