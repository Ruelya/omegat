import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from "electron";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { detectLocale, setLocale } from "../renderer/i18n";
import {
  createApplicationLifecycle,
  registerApplicationLifecycle,
} from "./lifecycle";
import { buildApplicationMenu } from "./menu";

type Pending = {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
};

let sidecar: ChildProcessWithoutNullStreams | null = null;
const pending = new Map<number, Pending>();
let nextId = 1;
let buf = "";

if (process.env.OMEGAT_CONFIG_DIR) {
  app.setPath("userData", process.env.OMEGAT_CONFIG_DIR);
}

function sidecarName(): string {
  return process.platform === "win32" ? "omegat-sidecar.exe" : "omegat-sidecar";
}

function sidecarPath(): string {
  const name = sidecarName();
  const extra = join(process.resourcesPath, name);
  const dev = join(app.getAppPath(), "..", "..", "target", "debug", name);
  const rel = join(app.getAppPath(), "..", "..", "target", "release", name);
  if (existsSync(extra)) return extra;
  if (existsSync(rel)) return rel;
  return dev;
}

function manualPath(locale = "en"): string {
  const name = locale.startsWith("zh") ? "zh-CN.md" : "en.md";
  const bundled = join(process.resourcesPath, "manual", name);
  const javaHtml = join(process.resourcesPath, "manual", "java", "index.html");
  const dev = join(app.getAppPath(), "..", "..", "docs", "manual", name);
  if (existsSync(bundled)) return bundled;
  if (existsSync(javaHtml)) return javaHtml;
  return dev;
}

function startSidecar() {
  const bin = sidecarPath();
  sidecar = spawn(bin, [], { stdio: ["pipe", "pipe", "pipe"] });
  sidecar.stdout.setEncoding("utf8");
  sidecar.stdout.on("data", (chunk: string) => {
    buf += chunk;
    let idx: number;
    while ((idx = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, idx).trim();
      buf = buf.slice(idx + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line) as { id?: number; result?: unknown; error?: { message: string } };
        if (msg.id != null && pending.has(msg.id)) {
          const p = pending.get(msg.id)!;
          pending.delete(msg.id);
          if (msg.error) p.reject(new Error(msg.error.message));
          else p.resolve(msg.result);
        }
      } catch {
        /* ignore malformed */
      }
    }
  });
  sidecar.stderr.on("data", (d: Buffer) => {
    process.stderr.write(d);
  });
}

function stopSidecar() {
  sidecar?.kill();
  sidecar = null;
}

function rpc(method: string, params: unknown = {}): Promise<unknown> {
  if (!sidecar) startSidecar();
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    const line = JSON.stringify({ jsonrpc: "2.0", id, method, params });
    sidecar!.stdin.write(line + "\n");
  });
}

function createWindow() {
  const win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 960,
    minHeight: 640,
    backgroundColor: "#f4efe6",
    title: "OmegaT",
    webPreferences: {
      preload: join(__dirname, "../preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  if (process.env.ELECTRON_RENDERER_URL) {
    win.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    win.loadFile(join(__dirname, "../renderer/index.html"));
  }
  Menu.setApplicationMenu(buildApplicationMenu(win));
  win.webContents.on("did-finish-load", () => {
    win.webContents.send("menu:ready");
  });
}

function applyMenuLocale(locale: string) {
  setLocale(locale);
  const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
  if (win) Menu.setApplicationMenu(buildApplicationMenu(win));
}

app.whenReady().then(() => {
  startSidecar();
  ipcMain.handle("rpc", (_e, method: string, params: unknown) => rpc(method, params));
  ipcMain.handle("startup-context", () => ({
    project: process.env.OMEGAT_PROJECT || null,
    configDir: process.env.OMEGAT_CONFIG_DIR || app.getPath("userData"),
    scriptsDir: process.env.OMEGAT_SCRIPTS_DIR || null,
  }));
  ipcMain.handle("pick-dir", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] });
    return r.canceled ? null : r.filePaths[0];
  });
  ipcMain.handle("pick-file", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openFile"] });
    return r.canceled ? null : r.filePaths[0];
  });
  ipcMain.handle("pick-files", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openFile", "multiSelections"] });
    return r.canceled ? null : r.filePaths;
  });
  ipcMain.handle("save-text", async (_e, name: string, text: string) => {
    const r = await dialog.showSaveDialog({ defaultPath: name });
    if (r.canceled || !r.filePath) return null;
    writeFileSync(r.filePath, text, "utf8");
    return r.filePath;
  });
  registerApplicationLifecycle(
    ipcMain,
    createApplicationLifecycle(app, stopSidecar),
  );
  ipcMain.handle("open-path", async (_e, path: string) => {
    if (path) await shell.openPath(path);
  });
  ipcMain.handle("open-external", async (_e, url: string) => {
    if (url) await shell.openExternal(url);
  });
  ipcMain.handle("open-manual", async () => {
    const local = manualPath();
    if (existsSync(local)) await shell.openPath(local);
    else await shell.openExternal("https://omegat.org/manual");
  });
  ipcMain.handle("menu-locale", (_e, locale: string) => {
    applyMenuLocale(typeof locale === "string" ? locale : "en");
  });
  setLocale(detectLocale(process.env.OMEGAT_LOCALE || app.getLocale()));
  createWindow();
});

app.on("window-all-closed", () => {
  stopSidecar();
  if (process.platform !== "darwin") app.quit();
});
