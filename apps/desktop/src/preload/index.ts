import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown) => ipcRenderer.invoke("rpc", method, params ?? {}),
  pickDir: () => ipcRenderer.invoke("pick-dir") as Promise<string | null>,
  pickFile: () => ipcRenderer.invoke("pick-file") as Promise<string | null>,
  onMenu: (channel: string, fn: (...args: unknown[]) => void) => {
    const listener = (_: unknown, ...args: unknown[]) => fn(...args);
    ipcRenderer.on(channel, listener);
    return () => ipcRenderer.removeListener(channel, listener);
  },
});
