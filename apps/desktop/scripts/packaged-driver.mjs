// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";

export const WAIT_MS = 60_000;
export const desktopDir = resolve(import.meta.dirname, "..");

const defaultExecutable = {
  linux: join(desktopDir, "release", "linux-unpacked", "omegat-desktop"),
  win32: join(desktopDir, "release", "win-unpacked", "OmegaT.exe"),
  darwin: join(
    desktopDir,
    "release",
    process.arch === "arm64" ? "mac-arm64" : "mac",
    "OmegaT.app",
    "Contents",
    "MacOS",
    "OmegaT",
  ),
}[process.platform];

export const executable =
  process.env.OMEGAT_PACKAGED_EXECUTABLE ?? defaultExecutable;
export const sidecar =
  process.env.OMEGAT_SIDECAR
  ?? resolve(
    desktopDir,
    "..",
    "..",
    "target",
    "release",
    process.platform === "win32" ? "omegat-sidecar.exe" : "omegat-sidecar",
  );

export const sleep = (ms) =>
  new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

export async function waitFor(label, check, timeoutMs = WAIT_MS) {
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

export async function pathExists(path) {
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

export async function startPackagedDisplay(width = 1600, height = 1000) {
  if (process.platform !== "linux") {
    return { child: null, display: process.env.DISPLAY };
  }
  const child = spawn(
    "Xvfb",
    [
      "-displayfd",
      "3",
      "-screen",
      "0",
      `${width}x${height}x24`,
      "-nolisten",
      "tcp",
    ],
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

export async function stopPackagedDisplay(display) {
  if (!display?.child?.pid) return;
  try {
    display.child.kill("SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

async function pageTarget(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(1_000),
  });
  if (!response.ok) {
    throw new Error(`DevTools endpoint returned ${response.status}`);
  }
  const targets = await response.json();
  return targets.find((target) =>
    target.type === "page" && target.webSocketDebuggerUrl
  );
}

export class DevToolsClient {
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
        reject(new Error(
          `DevTools command timed out: ${method}${
            method === "Runtime.evaluate"
              ? ` ${String(params.expression).slice(0, 160)}`
              : ""
          }`,
        ));
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
        response.exceptionDetails.exception?.description
          ?? "renderer evaluation failed",
      );
    }
    return response.result?.value;
  }

  close() {
    this.socket.close();
  }
}

export async function descendants(rootPid) {
  if (process.platform !== "linux") return [];
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

export async function workspaceState(client) {
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

export function spawnPackagedApplication(
  display,
  configDir,
  project,
  extraEnv = {},
) {
  let stderr = "";
  const environment = {
    ...process.env,
    OMEGAT_CONFIG_DIR: configDir,
    ...extraEnv,
  };
  if (display) environment.DISPLAY = display;
  delete environment.OMEGAT_PROJECT;
  if (project) environment.OMEGAT_PROJECT = project;
  const application = spawn(executable, [
    "--disable-gpu",
    ...(process.platform === "linux" ? ["--no-sandbox"] : []),
  ], {
    detached: true,
    env: environment,
    stdio: ["ignore", "ignore", "pipe"],
  });
  application.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  return { application, client: null, stderr: () => stderr };
}

export async function launchPackagedRenderer(
  display,
  configDir,
  project,
  extraEnv = {},
) {
  const port = await unusedPort();
  let stderr = "";
  const environment = {
    ...process.env,
    OMEGAT_CONFIG_DIR: configDir,
    ...extraEnv,
  };
  if (display) environment.DISPLAY = display;
  delete environment.OMEGAT_PROJECT;
  if (project) environment.OMEGAT_PROJECT = project;
  const args = [`--remote-debugging-port=${port}`, "--disable-gpu"];
  if (process.platform === "linux") args.push("--no-sandbox");
  const application = spawn(executable, args, {
    detached: true,
    env: environment,
    stdio: ["ignore", "ignore", "pipe"],
  });
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

export async function launchPackaged(
  display,
  configDir,
  project,
  extraEnv = {},
) {
  const launched = await launchPackagedRenderer(
    display,
    configDir,
    project,
    extraEnv,
  );
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

async function pidAlive(pid) {
  if (process.platform === "linux") return pathExists(`/proc/${pid}`);
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error.code === "ESRCH") return false;
    throw error;
  }
}

async function taskkill(pid) {
  const child = spawn("taskkill", ["/PID", String(pid), "/T", "/F"], {
    stdio: ["ignore", "ignore", "pipe"],
  });
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const code = await new Promise((resolveExit, reject) => {
    child.once("error", reject);
    child.once("exit", resolveExit);
  });
  assert(
    code === 0 || !(await pidAlive(pid)),
    `taskkill failed with ${code}: ${stderr}`,
  );
}

export async function killPackaged(launched) {
  const browserPid = launched.application.pid;
  const processes = await descendants(browserPid);
  const sidecarProcess = processes.find(({ command }) =>
    command.includes("omegat-sidecar")
  );
  if (process.platform === "linux") {
    assert(sidecarProcess, `packaged sidecar not found: ${JSON.stringify(processes)}`);
  }
  if (process.platform === "win32") await taskkill(browserPid);
  else process.kill(-browserPid, "SIGKILL");
  await waitFor("killed packaged Electron", async () => !await pidAlive(browserPid));
  if (sidecarProcess) {
    await waitFor(
      "killed packaged sidecar",
      async () => !await pidAlive(sidecarProcess.pid),
    );
  }
  launched.client.close();
  return { browserPid, sidecarPid: sidecarProcess?.pid ?? null };
}

export async function killPackagedProcess(launched) {
  return killPackaged({
    ...launched,
    client: launched.client ?? { close() {} },
  });
}

export async function terminatePackaged(launched) {
  if (!launched?.application?.pid) return;
  launched.client?.close();
  const pid = launched.application.pid;
  try {
    if (process.platform === "win32") await taskkill(pid);
    else process.kill(-pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
  try {
    await waitFor("terminated packaged Electron", async () => !await pidAlive(pid), 5_000);
  } catch {
    if (process.platform === "win32") await taskkill(pid);
    else {
      try {
        process.kill(-pid, "SIGKILL");
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
      }
    }
    await waitFor("killed packaged Electron cleanup", async () => !await pidAlive(pid));
  }
}
