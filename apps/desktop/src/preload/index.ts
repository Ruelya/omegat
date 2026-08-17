import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown) => ipcRenderer.invoke("rpc", method, params ?? {}),
  pickDir: () => ipcRenderer.invoke("pick-dir") as Promise<string | null>,
  pickFile: () => ipcRenderer.invoke("pick-file") as Promise<string | null>,
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
