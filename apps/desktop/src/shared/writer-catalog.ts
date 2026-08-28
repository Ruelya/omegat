// SPDX-License-Identifier: GPL-3.0-or-later

import rpcRegistryJson from "../../../../crates/omegat-ipc/rpc-methods.json";

export type WriterScope = "project" | "config";
export type WriterActivation = "always" | "persist_true" | "recovery";
export type ReceiptRoute = "caller" | "recovery" | "config";

export type WriterCatalogEntry = {
  method: string;
  scope: WriterScope;
  activation: WriterActivation;
  receipt_route: ReceiptRoute;
  journal_operation: string;
  watches_project_inputs: boolean;
};

type RpcMethodRegistryEntry = {
  method: string;
  writer?: Omit<WriterCatalogEntry, "method">;
};

const rpcRegistry = rpcRegistryJson as {
  version: number;
  methods: RpcMethodRegistryEntry[];
};

export const RPC_REGISTRY_VERSION = rpcRegistry.version;
export const RPC_METHODS = Object.freeze(
  rpcRegistry.methods.map(({ method }) => method),
);
export const WRITER_CATALOG: readonly WriterCatalogEntry[] = Object.freeze(
  rpcRegistry.methods.flatMap(({ method, writer }) =>
    writer ? [{ method, ...writer }] : []
  ),
);

if (
  new Set(RPC_METHODS).size !== RPC_METHODS.length
  || new Set(WRITER_CATALOG.map(({ method }) => method)).size
    !== WRITER_CATALOG.length
  || WRITER_CATALOG.length !== 26
  || WRITER_CATALOG.filter(({ scope }) => scope === "project").length !== 22
  || WRITER_CATALOG.filter(({ scope }) => scope === "config").length !== 4
) {
  throw new Error("invalid canonical sidecar writer catalog");
}

const writersByMethod = new Map(
  WRITER_CATALOG.map((writer) => [writer.method, writer]),
);

export function writerForMethod(
  method: string,
): WriterCatalogEntry | undefined {
  return writersByMethod.get(method);
}

export function isWriterActive(
  writer: WriterCatalogEntry,
  params: unknown,
): boolean {
  if (writer.activation === "always") return true;
  if (writer.activation === "recovery") return false;
  return params !== null
    && typeof params === "object"
    && "persist" in params
    && params.persist === true;
}

export function isProjectTransactionMethod(method: string): boolean {
  const writer = writerForMethod(method);
  return writer?.scope === "project" && writer.activation === "always";
}

export function isConfigTransactionMethodFromCatalog(
  method: string,
  params: unknown,
): boolean {
  const writer = writerForMethod(method);
  return Boolean(
    writer
      && writer.scope === "config"
      && isWriterActive(writer, params),
  );
}

export function isCallerManagedReceiptMethod(method: string): boolean {
  return writerForMethod(method)?.receipt_route === "caller";
}

export function watchesProjectInputs(method: string): boolean {
  return writerForMethod(method)?.watches_project_inputs === true;
}
