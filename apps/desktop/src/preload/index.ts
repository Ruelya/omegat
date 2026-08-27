import { contextBridge, ipcRenderer, webUtils } from "electron";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown) => ipcRenderer.invoke("rpc", method, params ?? {}),
  startup: () =>
    ipcRenderer.invoke("startup-context") as Promise<{
      project: string | null;
      configDir: string;
      scriptsDir: string | null;
    }>,
  pickDir: () => ipcRenderer.invoke("pick-dir") as Promise<string | null>,
  pickFile: () => ipcRenderer.invoke("pick-file") as Promise<string | null>,
  pickFiles: () => ipcRenderer.invoke("pick-files") as Promise<string[] | null>,
  pathForFile: (file: File) => webUtils.getPathForFile(file),
  inspectDrop: (paths: string[]) =>
    ipcRenderer.invoke("inspect-drop", paths) as Promise<
      | { kind: "project"; root: string }
      | { kind: "files"; paths: string[] }
    >,
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
