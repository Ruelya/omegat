import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown) => ipcRenderer.invoke("rpc", method, params ?? {}),
  pickDir: () => ipcRenderer.invoke("pick-dir") as Promise<string | null>,
  pickFile: () => ipcRenderer.invoke("pick-file") as Promise<string | null>,
  pickFiles: () => ipcRenderer.invoke("pick-files") as Promise<string[] | null>,
  saveText: (name: string, text: string) => ipcRenderer.invoke("save-text", name, text) as Promise<string | null>,
  quit: () => ipcRenderer.invoke("app-quit") as Promise<void>,
  relaunch: () => ipcRenderer.invoke("app-relaunch") as Promise<void>,
  openPath: (path: string) => ipcRenderer.invoke("open-path", path) as Promise<void>,
  openExternal: (url: string) => ipcRenderer.invoke("open-external", url) as Promise<void>,
  openManual: () => ipcRenderer.invoke("open-manual") as Promise<void>,
  setMenuLocale: (locale: string) => ipcRenderer.invoke("menu-locale", locale) as Promise<void>,
  onMenu: (channel: string, fn: (...args: unknown[]) => void) => {
    const listener = (_: unknown, ...args: unknown[]) => fn(...args);
    ipcRenderer.on(channel, listener);
    return () => ipcRenderer.removeListener(channel, listener);
  },
});
