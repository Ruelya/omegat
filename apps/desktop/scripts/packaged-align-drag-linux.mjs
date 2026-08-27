// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { access, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

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

if (process.platform !== "linux") {
  throw new Error("This E2E exercises real pointer input in a Linux package");
}
await access(executable);

const workDir = await mkdtemp(join(tmpdir(), "omegat-align-drag-e2e-"));
const configDir = join(workDir, "config");
const source = join(workDir, "source.txt");
const target = join(workDir, "target.txt");
const sourceLines = Array.from(
  { length: 80 },
  (_, index) => `Source sentence ${String(index).padStart(2, "0")}.`,
);
const targetLines = Array.from(
  { length: 80 },
  (_, index) => `Phrase cible ${String(index).padStart(2, "0")}.`,
);
await writeFile(source, sourceLines.join("\n\n"), "utf8");
await writeFile(target, targetLines.join("\n\n"), "utf8");

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
  await waitFor("renderer preload", async () =>
    (await client.evaluate("typeof window.omegat?.rpc")) === "function"
      ? true
      : undefined,
  );

  // A fresh config opens Tip of the Day. Close it before exercising the actual
  // native application menu that owns the aligner action.
  await client.evaluate(
    "document.querySelectorAll('.modal-bg').forEach((modal) => modal.click())",
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
  const alignerIsOpen = () =>
    client.evaluate(
      "Boolean(document.querySelector('table[aria-label=\"manual alignment table\"]'))",
    );
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
  const windowX = Number(geometry.X);
  const windowY = Number(geometry.Y);
  // Electron's Linux menu is native chrome, outside the DevTools page target.
  // Exercise it through XTEST at deterministic coordinates in the fixed
  // 1440x900 package window: Tools, then its Aligner row.
  await xdotool(xvfb.display, [
    "mousemove",
    String(windowX + 230),
    String(windowY + 10),
    "click",
    "1",
  ]);
  await sleep(200);
  await xdotool(xvfb.display, [
    "mousemove",
    String(windowX + 250),
    String(windowY + 225),
    "click",
    "1",
  ]);
  await sleep(200);
  assert.equal(
    await alignerIsOpen(),
    true,
    "The packaged native Tools menu did not open the aligner",
  );

  await client.evaluate(`(() => {
    const setValue = (selector, value) => {
      const input = document.querySelector(selector);
      const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      ).set;
      setter.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    };
    setValue('input[placeholder="source"]', ${JSON.stringify(source)});
    setValue('input[placeholder="target"]', ${JSON.stringify(target)});
    [...document.querySelectorAll(".modal button")].find(
      (button) => button.classList.contains("primary"),
    ).click();
  })()`);
  const initial = await waitFor("80 rendered alignment rows", async () => {
    const state = await client.evaluate(`(() => {
      const viewport = document.querySelector(".align-table-scroll");
      const rows = document.querySelectorAll("tr[data-align-row]");
      const first = document.querySelector("#align-cell-0-source");
      if (!viewport || rows.length !== 80 || !first) return null;
      // The aligner form is taller than the modal's own scrollport. Reveal
      // the complete inner row viewport before mapping DOM points to XTEST;
      // otherwise its lower edge can exist below the physical Xvfb screen.
      viewport.scrollIntoView({ block: "end" });
      viewport.scrollTop = 0;
      const start = first.getBoundingClientRect();
      const edge = viewport.getBoundingClientRect();
      if (edge.top < 0 || edge.bottom > window.innerHeight) return null;
      return {
        rowCount: rows.length,
        sourceText: first.textContent,
        scrollTop: viewport.scrollTop,
        startX: start.left + start.width / 2,
        startY: start.top + start.height / 2,
        edgeX: start.left + start.width / 2,
        // Stay above Chromium's horizontal scrollbar while remaining inside
        // the renderer's 48px autoscroll pressure zone.
        edgeY: edge.bottom - 24,
      };
    })()`);
    return state?.rowCount === 80 ? state : undefined;
  });
  assert.equal(initial.scrollTop, 0);
  assert.equal(initial.sourceText, sourceLines[0]);

  const pageMetrics = await client.evaluate(`({
    outerWidth: window.outerWidth,
    outerHeight: window.outerHeight,
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
  })`);
  const contentLeft =
    windowX + Math.max(0, (pageMetrics.outerWidth - pageMetrics.innerWidth) / 2);
  const contentTop =
    windowY + Math.max(0, pageMetrics.outerHeight - pageMetrics.innerHeight);
  let startScreenX = Math.round(contentLeft + initial.startX);
  let startScreenY = Math.round(contentTop + initial.startY);
  let edgeScreenX = Math.round(contentLeft + initial.edgeX);
  let edgeScreenY = Math.round(contentTop + initial.edgeY);
  await client.evaluate(`(() => {
    window.__omegatE2ePointer = null;
    window.__omegatE2eDragEvents = [];
    document.addEventListener("pointermove", (event) => {
      window.__omegatE2ePointer = {
        clientX: event.clientX,
        clientY: event.clientY,
        target: event.target?.id ?? null,
      };
    });
    for (const type of ["pointerdown", "mousedown", "dragstart", "dragover"]) {
      document.addEventListener(type, (event) => {
        window.__omegatE2eDragEvents.push({
          type,
          target: event.target?.id ?? null,
          clientX: event.clientX,
          clientY: event.clientY,
          buttons: event.buttons,
        });
      });
    }
  })()`);
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(startScreenX),
    String(startScreenY),
  ]);
  const observedPointer = await waitFor("XTEST pointer calibration", () =>
    client.evaluate("window.__omegatE2ePointer"),
  );
  const correctionX = Math.round(initial.startX - observedPointer.clientX);
  const correctionY = Math.round(initial.startY - observedPointer.clientY);
  startScreenX += correctionX;
  startScreenY += correctionY;
  edgeScreenX += correctionX;
  edgeScreenY += correctionY;
  await client.evaluate("window.__omegatE2ePointer = null");
  await xdotool(xvfb.display, [
    "mousemove",
    "--sync",
    String(startScreenX),
    String(startScreenY),
  ]);
  const calibratedPointer = await waitFor("source-cell pointer target", async () => {
    const pointer = await client.evaluate("window.__omegatE2ePointer");
    return pointer?.target === "align-cell-0-source" ? pointer : undefined;
  });
  assert.equal(calibratedPointer.target, "align-cell-0-source");
  await xdotool(xvfb.display, [
    "mousedown",
    "1",
  ]);
  await sleep(150);
  await xdotool(xvfb.display, [
    "mousemove_relative",
    "24",
    "8",
  ]);
  await sleep(500);
  const dragStartEvents = await client.evaluate("window.__omegatE2eDragEvents");
  assert(
    dragStartEvents.some((event) => event.type === "dragstart"),
    `XTEST did not initiate a native drag: ${JSON.stringify(dragStartEvents)}`,
  );
  const dragStartX = startScreenX + 24;
  const dragStartY = startScreenY + 8;
  for (let step = 1; step <= 12; step += 1) {
    const x = Math.round(
      dragStartX + ((edgeScreenX - dragStartX) * step) / 12,
    );
    const y = Math.round(
      dragStartY + ((edgeScreenY - dragStartY) * step) / 12,
    );
    await xdotool(xvfb.display, ["mousemove", String(x), String(y)]);
    await sleep(25);
  }
  await sleep(500);
  const afterMotion = await client.evaluate(`(() => {
    const viewport = document.querySelector(".align-table-scroll");
    return {
      events: window.__omegatE2eDragEvents.slice(-20),
      pointer: window.__omegatE2ePointer,
      scrollTop: viewport?.scrollTop ?? 0,
    };
  })()`);
  assert(
    afterMotion.events.some((event) => event.type === "dragover"),
    `Native drag never reached the viewport: ${JSON.stringify(afterMotion)}`,
  );
  assert(
    afterMotion.scrollTop > 0,
    `Native drag missed the viewport edge: ${JSON.stringify({
      desiredEdge: { x: initial.edgeX, y: initial.edgeY },
      ...afterMotion,
    })}`,
  );

  const hovered = await waitFor("stationary pointer drag autoscroll", async () => {
    const state = await client.evaluate(`(() => {
      const viewport = document.querySelector(".align-table-scroll");
      const table = document.querySelector(
        'table[aria-label="manual alignment table"]',
      );
      return {
        scrollTop: viewport?.scrollTop ?? 0,
        maxScrollTop: viewport
          ? viewport.scrollHeight - viewport.clientHeight
          : 0,
        activeDescendant: table?.getAttribute("aria-activedescendant"),
        target: document.querySelector(".drag-target")?.id ?? null,
      };
    })()`);
    return state.scrollTop > 0 &&
      state.activeDescendant !== "align-cell-0-source" &&
      /^align-(cell|drop-bottom)-/.test(state.target ?? "")
      ? state
      : undefined;
  });
  assert(hovered.maxScrollTop > 0);
  assert.notEqual(hovered.activeDescendant, "align-cell-0-source");
  assert.match(hovered.target ?? "", /^align-(cell|drop-bottom)-/);

  await waitFor("bottom drag boundary", async () => {
    const active = await client.evaluate(
      `document.querySelector('table[aria-label="manual alignment table"]')
        ?.getAttribute("aria-activedescendant")`,
    );
    return active === "align-drop-bottom-source" ? true : undefined;
  });
  const bottomDropPoint = await client.evaluate(`(() => {
    const cell = document.querySelector("#align-drop-bottom-source");
    const rect = cell?.getBoundingClientRect();
    if (!rect) return null;
    return {
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + Math.min(2, rect.height / 2),
      hit: document.elementFromPoint(
        rect.left + rect.width / 2,
        rect.top + Math.min(2, rect.height / 2),
      )?.id ?? null,
    };
  })()`);
  assert.equal(bottomDropPoint?.hit, "align-drop-bottom-source");
  await xdotool(xvfb.display, [
    "mousemove",
    String(Math.round(contentLeft + bottomDropPoint.clientX + correctionX)),
    String(Math.round(contentTop + bottomDropPoint.clientY + correctionY)),
  ]);
  // At maximum scroll the boundary can move under an otherwise stationary
  // pointer. Jiggle inside the same wide cell so Chromium emits a fresh native
  // dragover whose real event target is the explicit boundary.
  await sleep(50);
  await xdotool(xvfb.display, ["mousemove_relative", "4", "0"]);
  await sleep(50);
  await xdotool(xvfb.display, ["mousemove_relative", "--", "-4", "0"]);
  await waitFor("native dragover on bottom drop cell", async () => {
    const events = await client.evaluate("window.__omegatE2eDragEvents");
    return events.some(
      (event) =>
        event.type === "dragover" &&
        event.target === "align-drop-bottom-source",
    )
      ? true
      : undefined;
  });
  await xdotool(xvfb.display, ["mouseup", "1"]);

  const dropped = await waitFor("sidecar-backed drop result", async () => {
    const state = await client.evaluate(`(() => {
      const rows = document.querySelectorAll("tr[data-align-row]");
      return {
        rowCount: rows.length,
        firstSource: document.querySelector("#align-cell-0-source")?.textContent,
        lastSource: document.querySelector(
          "#align-cell-" + (rows.length - 1) + "-source",
        )?.textContent,
        focused:
          document.activeElement?.getAttribute("aria-label") ===
          "manual alignment table",
      };
    })()`);
    return state.lastSource === sourceLines[0] ? state : undefined;
  });
  assert.equal(dropped.rowCount, 80);
  assert.equal(dropped.firstSource, sourceLines[1]);
  assert.equal(dropped.lastSource, sourceLines[0]);
  assert.equal(dropped.focused, true);

  console.log(
    JSON.stringify({
      result: "passed",
      package: executable,
      rows: dropped.rowCount,
      initialScrollTop: initial.scrollTop,
      hoveredScrollTop: hovered.scrollTop,
      activeDescendant: hovered.activeDescendant,
      pointerInput: "XTEST",
      movedSource: {
        from: 0,
        to: dropped.rowCount - 1,
      },
    }),
  );
  await client.evaluate('setTimeout(() => window.omegat.quit(), 0); "quit"');
} catch (error) {
  if (stderr) process.stderr.write(stderr);
  throw error;
} finally {
  client?.close();
  terminate(launched?.pid);
  terminate(xvfb.child.pid);
  await rm(workDir, { recursive: true, force: true });
}
