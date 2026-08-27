// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const WAIT_MS = 30_000;
const POLL_MS = 100;
const desktopDir = resolve(import.meta.dirname, "..");
const executable =
  process.env.OMEGAT_PACKAGED_EXECUTABLE ??
  join(desktopDir, "release", "linux-unpacked", "omegat-desktop");
const sidecar =
  process.env.OMEGAT_SIDECAR ??
  resolve(desktopDir, "..", "..", "target", "release", "omegat-sidecar");

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
  return targets.find(
    (target) => target.type === "page" && target.webSocketDebuggerUrl,
  );
}

class DevToolsClient {
  constructor(url) {
    this.socket = new WebSocket(url);
    this.nextId = 1;
    this.pending = new Map();
    this.events = [];
  }

  async connect() {
    await new Promise((resolveOpen, reject) => {
      this.socket.addEventListener("open", resolveOpen, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id == null) {
        this.events.push(message);
        return;
      }
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
    const result = await this.command("Runtime.evaluate", {
      expression,
      awaitPromise,
      returnByValue: true,
    });
    if (result.exceptionDetails) {
      throw new Error(
        result.exceptionDetails.exception?.description ??
          "Renderer evaluation failed",
      );
    }
    return result.result?.value;
  }

  close() {
    this.socket.close();
  }
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

function terminate(pid) {
  if (!pid) return;
  try {
    process.kill(pid, "SIGTERM");
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

async function terminateAndWait(child) {
  if (!child || child.exitCode != null || child.signalCode != null) return;
  const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
  terminate(child.pid);
  await Promise.race([exited, sleep(2_000)]);
}

if (process.platform !== "linux") {
  throw new Error("This E2E exercises real editor input in a Linux package");
}
await Promise.all([access(executable), access(sidecar)]);

const workDir = await mkdtemp(join(tmpdir(), "omegat-editor-ime-e2e-"));
const configDir = join(workDir, "config");
const projectDir = join(workDir, "project");
await mkdir(join(projectDir, "source"), { recursive: true });
await mkdir(configDir, { recursive: true });
await writeFile(
  join(projectDir, "source", "editor.txt"),
  "Editor input selection source.",
  "utf8",
);
await rpcOnce(configDir, "project.create", {
  root: projectDir,
  source_lang: "en",
  target_lang: "ja",
  sentence_seg: false,
});

const port = await unusedPort();
const xvfb = await startXvfb();
let launched;
let client;
let stderr = "";

try {
  launched = spawn(
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
  launched.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  const targetInfo = await waitFor("packaged renderer", () => pageTarget(port));
  client = new DevToolsClient(targetInfo.webSocketDebuggerUrl);
  await client.connect();
  await Promise.all([
    client.command("Runtime.enable"),
    client.command("Log.enable"),
  ]);
  const readyEditor = await waitFor("active editor product surface", async () => {
    const ready = await client.evaluate(`(() => {
      document.querySelectorAll(".modal-bg").forEach((modal) => modal.click());
      return {
        preload: typeof window.omegat?.rpc,
        source: document.querySelector(".editor-segment.is-active .src")?.textContent,
        surface: Boolean(document.querySelector(".editor-surface")),
        body: document.body.innerText.slice(0, 500),
        html: document.body.innerHTML.slice(0, 500),
      };
    })()`);
    const failures = client.events
      .filter((event) =>
        event.method === "Runtime.exceptionThrown" ||
        event.method === "Log.entryAdded"
      )
      .slice(-5);
    if (ready.preload === "function" && ready.surface) return ready;
    throw new Error(JSON.stringify({ ready, failures }));
  });
  assert.equal(
    readyEditor.source,
    "Editor input selection source.",
    JSON.stringify(readyEditor),
  );

  const windowId = (
    await waitFor("OmegaT X11 window", async () => {
      const ids = await xdotool(xvfb.display, [
        "search",
        "--sync",
        "--onlyvisible",
        "--name",
        "OmegaT",
      ]);
      return ids.split(/\s+/).filter(Boolean).at(-1);
    })
  ).toString();
  await xdotool(xvfb.display, ["windowfocus", "--sync", windowId]);
  const geometry = Object.fromEntries(
    (
      await xdotool(xvfb.display, [
        "getwindowgeometry",
        "--shell",
        windowId,
      ])
    )
      .split("\n")
      .map((line) => line.split("=")),
  );
  const pageMetrics = await client.evaluate(`({
    outerWidth: window.outerWidth,
    outerHeight: window.outerHeight,
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
  })`);
  const contentLeft =
    Number(geometry.X) +
    Math.max(0, (pageMetrics.outerWidth - pageMetrics.innerWidth) / 2);
  const contentTop =
    Number(geometry.Y) +
    Math.max(0, pageMetrics.outerHeight - pageMetrics.innerHeight);
  const focusPoint = await client.evaluate(`(() => {
    const rect = document.querySelector(".editor-surface").getBoundingClientRect();
    return { x: rect.left + 20, y: rect.top + 20 };
  })()`);
  await client.evaluate(`(() => {
    window.__omegatE2ePointer = null;
    window.__omegatE2eImeEvents = [];
    window.__omegatE2eMouseDrag = {
      sequence: [],
      trusted: [],
      buttons: [],
    };
    document.addEventListener("pointermove", (event) => {
      window.__omegatE2ePointer = {
        clientX: event.clientX,
        clientY: event.clientY,
        editor: Boolean(event.target?.closest?.(".editor-surface")),
      };
    });
    for (const type of ["compositionstart", "compositionupdate", "compositionend", "beforeinput"]) {
      document.addEventListener(type, (event) => {
        window.__omegatE2eImeEvents.push({
          type,
          data: event.data ?? null,
          inputType: event.inputType ?? null,
          composing: event.isComposing ?? null,
        });
      }, true);
    }
    for (const type of ["mousedown", "mousemove", "mouseup"]) {
      document.addEventListener(type, (event) => {
        const drag = window.__omegatE2eMouseDrag;
        const editor = Boolean(event.target?.closest?.(".editor-surface"));
        if (type === "mousedown" && editor && event.button === 0) {
          drag.sequence = ["mousedown"];
          drag.trusted = [event.isTrusted];
          drag.buttons = [event.buttons];
          return;
        }
        if (
          type === "mousemove"
          && drag.sequence.length === 1
          && editor
          && (event.buttons & 1) === 1
        ) {
          drag.sequence.push("mousemove");
          drag.trusted.push(event.isTrusted);
          drag.buttons.push(event.buttons);
          return;
        }
        if (
          type === "mouseup"
          && drag.sequence.length === 2
          && editor
          && event.button === 0
        ) {
          drag.sequence.push("mouseup");
          drag.trusted.push(event.isTrusted);
          drag.buttons.push(event.buttons);
        }
      }, true);
    }
  })()`);

  let focusScreenX = Math.round(contentLeft + focusPoint.x);
  let focusScreenY = Math.round(contentTop + focusPoint.y);
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(focusScreenX),
    String(focusScreenY),
  ]);
  const observed = await waitFor("XTEST pointer calibration", () =>
    client.evaluate("window.__omegatE2ePointer"),
  );
  const correctionX = Math.round(focusPoint.x - observed.clientX);
  const correctionY = Math.round(focusPoint.y - observed.clientY);
  focusScreenX += correctionX;
  focusScreenY += correctionY;
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(focusScreenX),
    String(focusScreenY),
    "click",
    "1",
  ]);
  assert.equal(
    await client.evaluate("document.activeElement?.classList.contains('ime-proxy')"),
    true,
    "The real surface click did not focus the native IME proxy",
  );

  await client.command("Input.insertText", { text: "alpha 😀 beta" });
  assert.equal(
    await waitFor("native beforeinput text", async () => {
      const state = await client.evaluate(`({
        text: document.querySelector(".editor-surface")?.textContent ?? null,
        proxyValue: document.querySelector(".ime-proxy")?.value ?? null,
        active: document.activeElement?.className ?? null,
        events: window.__omegatE2eImeEvents,
      })`);
      if (state.text === "alpha 😀 beta") return state.text;
      throw new Error(JSON.stringify(state));
    }),
    "alpha 😀 beta",
  );

  const selectionPoints = await client.evaluate(`(() => {
    const fragments = [...document.querySelectorAll(".editor-surface [data-offset]")];
    const point = (offset) => {
      const fragment = fragments.find((element) => {
        const start = Number(element.dataset.offset);
        return offset >= start && offset <= start + element.textContent.length;
      });
      const start = Number(fragment.dataset.offset);
      const node = fragment.firstChild;
      const range = document.createRange();
      range.setStart(node, offset - start);
      range.collapse(true);
      const caret = range.getBoundingClientRect();
      const bounds = fragment.getBoundingClientRect();
      return { x: caret.left, y: bounds.top + bounds.height / 2 };
    };
    return { start: point(0), end: point(5) };
  })()`);
  const screenPoint = (point) => ({
    x: Math.round(contentLeft + point.x + correctionX),
    y: Math.round(contentTop + point.y + correctionY),
  });
  const selectionStart = screenPoint(selectionPoints.start);
  const selectionEnd = screenPoint(selectionPoints.end);
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(selectionStart.x),
    String(selectionStart.y),
    "mousedown",
    "1",
  ]);
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(selectionEnd.x),
    String(selectionEnd.y),
  ]);
  await xdotool(xvfb.display, [
    "mouseup",
    "1",
  ]);
  const selected = await waitFor("native XTEST mouse drag editor selection", async () => {
    const state = await client.evaluate(`(() => {
      const selection = document.querySelector(".editor-selection");
      return {
        text: selection?.textContent ?? null,
        caretAfter: selection?.nextElementSibling?.classList.contains("caret") ?? false,
        focusedProxy: document.activeElement?.classList.contains("ime-proxy") ?? false,
        drag: window.__omegatE2eMouseDrag,
      };
    })()`);
    return state.text === "alpha" && state.drag.sequence.length === 3 ? state : undefined;
  });
  assert.deepEqual(selected, {
    text: "alpha",
    caretAfter: true,
    focusedProxy: true,
    drag: {
      sequence: ["mousedown", "mousemove", "mouseup"],
      trusted: [true, true, true],
      buttons: [1, 1, 0],
    },
  });

  await client.evaluate("window.__omegatE2eImeEvents = []");
  await client.command("Input.imeSetComposition", {
    text: "に",
    selectionStart: 1,
    selectionEnd: 1,
  });
  await client.command("Input.imeSetComposition", {
    text: "日本語",
    selectionStart: 3,
    selectionEnd: 3,
  });
  await client.command("Input.insertText", { text: "日本語" });
  const composed = await waitFor("committed replaceable IME composition", async () => {
    const state = await client.evaluate(`(() => {
      const events = window.__omegatE2eImeEvents;
      return {
        text: document.querySelector(".editor-surface")?.textContent ?? null,
        compositionStarts: events.filter((event) => event.type === "compositionstart").length,
        compositionEnds: events.filter((event) => event.type === "compositionend").length,
        firstUpdate: events.find((event) => event.type === "compositionupdate")?.data ?? null,
        lastUpdate: events.filter((event) => event.type === "compositionupdate").at(-1)?.data ?? null,
        beforeInputTypes: events
          .filter((event) => event.type === "beforeinput")
          .map((event) => event.inputType),
        events,
      };
    })()`);
    if (state.text === "日本語 😀 beta") return state;
    throw new Error(JSON.stringify(state));
  });
  assert.equal(composed.compositionStarts, 2);
  assert.equal(composed.compositionEnds, 0);
  assert.equal(composed.firstUpdate, "に");
  assert.equal(composed.lastUpdate, "日本語");

  await client.evaluate("window.__omegatE2eImeEvents = []");
  await client.command("Input.imeSetComposition", {
    text: "失焦",
    selectionStart: 2,
    selectionEnd: 2,
  });
  await xdotool(xvfb.display, ["key", "Tab"]);
  const blurred = await waitFor("IME composition committed on native focus loss", async () => {
    const state = await client.evaluate(`(() => {
      const events = window.__omegatE2eImeEvents;
      return {
        text: document.querySelector(".editor-surface")?.textContent ?? null,
        focusedProxy: document.activeElement?.classList.contains("ime-proxy") ?? false,
        compositionStarts: events.filter((event) => event.type === "compositionstart").length,
        compositionEnds: events.filter((event) => event.type === "compositionend").length,
        beforeInputTypes: events
          .filter((event) => event.type === "beforeinput")
          .map((event) => event.inputType),
      };
    })()`);
    if (state.text === "日本語失焦 😀 beta" && !state.focusedProxy) return state;
    throw new Error(JSON.stringify(state));
  });
  assert.deepEqual(blurred, {
    text: "日本語失焦 😀 beta",
    focusedProxy: false,
    compositionStarts: 1,
    compositionEnds: 0,
    beforeInputTypes: ["insertCompositionText"],
  });

  const endPoint = await client.evaluate(`(() => {
    const fragments = [...document.querySelectorAll(".editor-surface [data-offset]")];
    const fragment = fragments.reduce((latest, candidate) => {
      const latestEnd = Number(latest.dataset.offset) + latest.textContent.length;
      const candidateEnd = Number(candidate.dataset.offset) + candidate.textContent.length;
      return candidateEnd > latestEnd ? candidate : latest;
    });
    const node = fragment.firstChild;
    const range = document.createRange();
    range.setStart(node, node.textContent.length);
    range.collapse(true);
    const caret = range.getBoundingClientRect();
    const bounds = fragment.getBoundingClientRect();
    return { x: caret.left, y: bounds.top + bounds.height / 2 };
  })()`);
  const endScreen = screenPoint(endPoint);
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(endScreen.x),
    String(endScreen.y),
    "click",
    "1",
  ]);
  assert.equal(
    await client.evaluate("document.activeElement?.classList.contains('ime-proxy')"),
    true,
    "The real surface click did not restore native IME proxy focus",
  );

  await client.evaluate("window.__omegatE2eImeEvents = []");
  await client.command("Input.imeSetComposition", {
    text: "取消中",
    selectionStart: 3,
    selectionEnd: 3,
  });
  assert.equal(
    await waitFor("native IME cancellation candidate", async () => {
      const text = await client.evaluate(
        "document.querySelector('.editor-surface')?.textContent ?? null",
      );
      return text === "日本語失焦 😀 beta取消中" ? text : undefined;
    }),
    "日本語失焦 😀 beta取消中",
  );
  await xdotool(xvfb.display, ["key", "Escape"]);
  const cancelled = await waitFor("IME composition restored by native Escape", async () => {
    const state = await client.evaluate(`(() => {
      const events = window.__omegatE2eImeEvents;
      return {
        text: document.querySelector(".editor-surface")?.textContent ?? null,
        focusedProxy: document.activeElement?.classList.contains("ime-proxy") ?? false,
        compositionStarts: events.filter((event) => event.type === "compositionstart").length,
        beforeInputTypes: events
          .filter((event) => event.type === "beforeinput")
          .map((event) => event.inputType),
      };
    })()`);
    if (state.text === "日本語失焦 😀 beta") return state;
    throw new Error(JSON.stringify(state));
  });
  assert.deepEqual(cancelled, {
    text: "日本語失焦 😀 beta",
    focusedProxy: true,
    compositionStarts: 1,
    beforeInputTypes: ["insertCompositionText"],
  });

  await xdotool(xvfb.display, ["key", "Return"]);
  const persisted = await waitFor("sidecar-backed editor commit", async () => {
    const entry = await client.evaluate(
      "window.omegat.rpc('entry.get', { index: 0 })",
      true,
    );
    return entry?.translation === "日本語失焦 😀 beta" ? entry : undefined;
  });
  assert.equal(persisted.translation, "日本語失焦 😀 beta");

  console.log(
    JSON.stringify({
      result: "passed",
      package: executable,
      pointerInput: "XTEST mousedown-mousemove-mouseup",
      mouseSequence: selected.drag.sequence,
      nativeInput: "Chromium Input.imeSetComposition",
      selected: selected.text,
      translation: persisted.translation,
      compositionStarts: composed.compositionStarts,
      compositionEnds: composed.compositionEnds,
      beforeInputTypes: composed.beforeInputTypes,
      blurCompositionStarts: blurred.compositionStarts,
      blurCompositionEnds: blurred.compositionEnds,
      blurBeforeInputTypes: blurred.beforeInputTypes,
      cancelCompositionStarts: cancelled.compositionStarts,
      cancelBeforeInputTypes: cancelled.beforeInputTypes,
      cancelRestored: cancelled.text === blurred.text,
    }),
  );
  await client.evaluate('setTimeout(() => window.omegat.quit(), 0); "quit"');
} catch (error) {
  if (stderr) process.stderr.write(stderr);
  throw error;
} finally {
  client?.close();
  await terminateAndWait(launched);
  await terminateAndWait(xvfb.child);
  await rm(workDir, { recursive: true, force: true });
}
