// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  isConfigTransactionMethod,
  scopeConfigTransaction,
} from "./config-transaction-scope";

describe("scopeConfigTransaction", () => {
  it("identifies every shared config writer", () => {
    expect(isConfigTransactionMethod("prefs.set", {})).toBe(true);
    expect(isConfigTransactionMethod("prefs.patch", {})).toBe(true);
    expect(isConfigTransactionMethod("spell.install", { lang: "en" })).toBe(true);
    expect(isConfigTransactionMethod("aligner.configure", { persist: true })).toBe(true);
    expect(isConfigTransactionMethod("aligner.configure", { persist: false })).toBe(false);
    expect(isConfigTransactionMethod("project.save", {})).toBe(false);
  });

  it("adds process identity without project scope", () => {
    expect(scopeConfigTransaction(
      "prefs.patch",
      { locale: "fr" },
      "electron-a",
      1234,
      () => "batch-a",
    )).toEqual({
      locale: "fr",
      config_transaction_app_instance: "electron-a",
      config_transaction_batch_id: "config-prefs.patch-batch-a",
      config_transaction_owner_process_id: 1234,
    });
    expect(scopeConfigTransaction(
      "aligner.configure",
      { persist: false },
      "electron-a",
      1234,
      () => "unused",
    )).toEqual({ persist: false });
  });

  it("preserves an explicit retry identity without leaking the hint", () => {
    expect(scopeConfigTransaction(
      "prefs.patch",
      {
        locale: "fr",
        config_transaction_retry_batch_id: "config-prefs.patch-original",
      },
      "electron-b",
      5678,
      () => "must-not-be-used",
    )).toEqual({
      locale: "fr",
      config_transaction_app_instance: "electron-b",
      config_transaction_batch_id: "config-prefs.patch-original",
      config_transaction_owner_process_id: 5678,
    });
  });
});
