import { contextBridge, ipcRenderer, webUtils } from "electron";
import type { RpcOperationEvent } from "../shared/rpc-operation";
import type {
  TransactionAck,
  TransactionEnvelope,
  TransactionOutcome,
} from "../shared/transaction-envelope";

contextBridge.exposeInMainWorld("omegat", {
  rpc: (method: string, params?: unknown, clientRequestId?: string) =>
    ipcRenderer.invoke("rpc", method, params ?? {}, clientRequestId),
  rpcWithTransactionReceipt: (
    method: string,
    params?: unknown,
    clientRequestId?: string,
  ) =>
    ipcRenderer.invoke(
      "rpc",
      method,
      params ?? {},
      clientRequestId,
      true,
    ) as Promise<unknown>,
  cancelRpc: (clientRequestId: string) =>
    ipcRenderer.invoke("rpc-cancel", clientRequestId) as Promise<boolean>,
  onRpcOperation: (fn: (event: RpcOperationEvent) => void) => {
    const listener = (_: unknown, event: RpcOperationEvent) => fn(event);
    ipcRenderer.on("rpc:operation", listener);
    return () => ipcRenderer.removeListener("rpc:operation", listener);
  },
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
  watchProject: (root: string, generation: number) =>
    ipcRenderer.invoke("project-watch", root, generation) as Promise<void>,
  unwatchProject: () => ipcRenderer.invoke("project-unwatch") as Promise<void>,
  acknowledgeTransactionReceipt: (
    envelope: TransactionEnvelope,
    outcome: TransactionOutcome = "succeeded",
  ) => ipcRenderer.invoke(
    "transaction-receipt-ack",
    envelope,
    outcome,
  ) as Promise<{
    ack: TransactionAck;
  }>,
  onTransactionEnvelope: (fn: (envelope: TransactionEnvelope) => void) => {
    const listener = (_: unknown, envelope: TransactionEnvelope) => fn(envelope);
    ipcRenderer.on("transaction:envelope", listener);
    return () => ipcRenderer.removeListener("transaction:envelope", listener);
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
