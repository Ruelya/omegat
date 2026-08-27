import { contextBridge, ipcRenderer, webUtils } from "electron";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown, clientRequestId?: string) =>
    ipcRenderer.invoke("rpc", method, params ?? {}, clientRequestId),
  cancelRpc: (clientRequestId: string) =>
    ipcRenderer.invoke("rpc-cancel", clientRequestId) as Promise<boolean>,
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
  watchProject: (root: string) => ipcRenderer.invoke("project-watch", root) as Promise<void>,
  unwatchProject: () => ipcRenderer.invoke("project-unwatch") as Promise<void>,
  onProjectExternalChange: (
    fn: (event: { root: string; paths: string[] }) => void,
  ) => {
    const listener = (
      _: unknown,
      event: { root: string; paths: string[] },
    ) => fn(event);
    ipcRenderer.on("project:external-change", listener);
    return () => ipcRenderer.removeListener("project:external-change", listener);
  },
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
