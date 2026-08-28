// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  isCallerManagedReceiptMethod,
  isConfigTransactionMethodFromCatalog,
  isProjectTransactionMethod,
  RPC_METHODS,
  RPC_REGISTRY_VERSION,
  watchesProjectInputs,
  WRITER_CATALOG,
} from "./writer-catalog";

describe("canonical sidecar writer catalog", () => {
  it("has one unique 22-project/4-config view of the RPC registry", () => {
    expect(RPC_REGISTRY_VERSION).toBe(1);
    expect(new Set(RPC_METHODS).size).toBe(RPC_METHODS.length);
    expect(WRITER_CATALOG).toHaveLength(26);
    expect(new Set(WRITER_CATALOG.map(({ method }) => method)).size).toBe(26);
    expect(WRITER_CATALOG.filter(({ scope }) => scope === "project")).toHaveLength(22);
    expect(WRITER_CATALOG.filter(({ scope }) => scope === "config")).toHaveLength(4);
  });

  it("derives transaction scoping, receipt routing, and watcher suppression", () => {
    for (const writer of WRITER_CATALOG) {
      expect(isCallerManagedReceiptMethod(writer.method)).toBe(
        writer.receipt_route === "caller",
      );
      expect(watchesProjectInputs(writer.method)).toBe(
        writer.watches_project_inputs,
      );
      expect(isProjectTransactionMethod(writer.method)).toBe(
        writer.scope === "project" && writer.activation === "always",
      );
      expect(isConfigTransactionMethodFromCatalog(
        writer.method,
        writer.activation === "persist_true" ? { persist: true } : {},
      )).toBe(writer.scope === "config");
    }
    expect(isConfigTransactionMethodFromCatalog(
      "aligner.configure",
      { persist: false },
    )).toBe(false);
  });
});
