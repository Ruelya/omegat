import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from "electron";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

type Pending = {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
};

let sidecar: ChildProcessWithoutNullStreams | None = null;
type None = null;
const pending = new Map<number, Pending>();
let nextId = 1;
let buf = "";

function sidecarPath(): string {
  const extra = join(process.resourcesPath, "omegat-sidecar");
  const dev = join(app.getAppPath(), "..", "..", "target", "debug", "omegat-sidecar");
  const rel = join(app.getAppPath(), "..", "..", "target", "release", "omegat-sidecar");
  if (existsSync(extra)) return extra;
  if (existsSync(rel)) return rel;
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
  Menu.setApplicationMenu(
    Menu.buildFromTemplate([
      {
        label: "Project",
        submenu: [
          {
            label: "Open…",
            accelerator: "CmdOrCtrl+O",
            click: async () => {
              const r = await dialog.showOpenDialog(win, { properties: ["openDirectory"] });
              if (!r.canceled && r.filePaths[0]) {
                win.webContents.send("menu:open", r.filePaths[0]);
              }
            },
          },
          {
            label: "Save",
            accelerator: "CmdOrCtrl+S",
            click: () => win.webContents.send("menu:save"),
          },
          {
            label: "Compile",
            accelerator: "CmdOrCtrl+D",
            click: () => win.webContents.send("menu:compile"),
          },
          { type: "separator" },
          { role: "quit" },
        ],
      },
      {
        label: "Edit",
        submenu: [
          { role: "undo" },
          { role: "redo" },
          { type: "separator" },
          { role: "cut" },
          { role: "copy" },
          { role: "paste" },
          {
            label: "Insert best match",
            accelerator: "CmdOrCtrl+I",
            click: () => win.webContents.send("menu:insert-match"),
          },
        ],
      },
      {
        label: "Go",
        submenu: [
          {
            label: "Next segment",
            accelerator: "CmdOrCtrl+N",
            click: () => win.webContents.send("menu:next"),
          },
          {
            label: "Previous segment",
            accelerator: "CmdOrCtrl+P",
            click: () => win.webContents.send("menu:prev"),
          },
        ],
      },
      {
        label: "Help",
        submenu: [
          {
            label: "Manual",
            click: () => shell.openExternal("https://omegat.org"),
          },
        ],
      },
    ]),
  );
}

app.whenReady().then(() => {
  startSidecar();
  ipcMain.handle("rpc", (_e, method: string, params: unknown) => rpc(method, params));
  ipcMain.handle("pick-dir", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] });
    return r.canceled ? null : r.filePaths[0];
  });
  ipcMain.handle("pick-file", async () => {
    const r = await dialog.showOpenDialog({ properties: ["openFile"] });
    return r.canceled ? null : r.filePaths[0];
  });
  createWindow();
});

app.on("window-all-closed", () => {
  sidecar?.kill();
  if (process.platform !== "darwin") app.quit();
});
