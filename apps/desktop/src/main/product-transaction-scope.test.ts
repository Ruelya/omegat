// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  scopeProductTransaction,
  transactionEnvelopesForRenderer,
  transactionReceiptIdentity,
} from "./product-transaction-scope";

describe("scopeProductTransaction", () => {
  it("binds every persistent project operation to the watched generation", () => {
    const project = { root: "/project", generation: 17 };
    let sequence = 0;
    const createId = () => `id-${++sequence}`;

    expect([
      scopeProductTransaction("entry.set", { index: 4 }, project, createId),
      scopeProductTransaction("project.save", undefined, project, createId),
      scopeProductTransaction("project.close", {}, project, createId),
      scopeProductTransaction("project.reload", {}, project, createId),
      scopeProductTransaction("project.compile", { file: "a.txt" }, project, createId),
      scopeProductTransaction("project.import", { files: ["/tmp/a.txt"] }, project, createId),
      scopeProductTransaction("project.update", { target_lang: "de" }, project, createId),
      scopeProductTransaction("team.mapping", { repositories: [] }, project, createId),
      scopeProductTransaction("team.sync", {}, project, createId),
      scopeProductTransaction("team.commit", { which: "target" }, project, createId),
      scopeProductTransaction("team.resolve", { source: "same" }, project, createId),
      scopeProductTransaction("glossary.add", { source: "cat" }, project, createId),
      scopeProductTransaction("search.replace", { query: "cat" }, project, createId),
      scopeProductTransaction("spell.ignore", { word: "OmegaT" }, project, createId),
      scopeProductTransaction("spell.learn", { word: "OmegaT" }, project, createId),
      scopeProductTransaction("tmx.export", { dest: "/tmp/out.tmx" }, project, createId),
      scopeProductTransaction("wiki.import", { source: "/tmp/wiki.xml" }, project, createId),
      scopeProductTransaction("script.run", { source: "project.save()" }, project, createId),
      scopeProductTransaction("script.slot", { slot: 1 }, project, createId),
      scopeProductTransaction("align.run", { dest: "/tmp/run.tmx" }, project, createId),
      scopeProductTransaction("align.write", { dest: "/tmp/write.tmx" }, project, createId),
    ]).toEqual([
      {
        index: 4,
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-entry.set-id-1",
      },
      {
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.save-id-2",
      },
      {
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.close-id-3",
      },
      {
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.reload-id-4",
      },
      {
        file: "a.txt",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.compile-id-5",
      },
      {
        files: ["/tmp/a.txt"],
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.import-id-6",
      },
      {
        target_lang: "de",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-project.update-id-7",
      },
      {
        repositories: [],
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-team.mapping-id-8",
      },
      {
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-team.sync-id-9",
      },
      {
        which: "target",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-team.commit-id-10",
      },
      {
        source: "same",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-team.resolve-id-11",
      },
      {
        source: "cat",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-glossary.add-id-12",
      },
      {
        query: "cat",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-search.replace-id-13",
      },
      {
        word: "OmegaT",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-spell.ignore-id-14",
      },
      {
        word: "OmegaT",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-spell.learn-id-15",
      },
      {
        dest: "/tmp/out.tmx",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-tmx.export-id-16",
      },
      {
        source: "/tmp/wiki.xml",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-wiki.import-id-17",
      },
      {
        source: "project.save()",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-script.run-id-18",
      },
      {
        slot: 1,
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-script.slot-id-19",
      },
      {
        dest: "/tmp/run.tmx",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-align.run-id-20",
      },
      {
        dest: "/tmp/write.tmx",
        transaction_project_root: "/project",
        transaction_generation: 17,
        transaction_batch_id: "product-align.write-id-21",
      },
    ]);
  });

  it("does not scope unrelated methods or writes without an active project", () => {
    const params = { query: "same" };
    expect(scopeProductTransaction("search.run", params, {
      root: "/project",
      generation: 1,
    }, () => "unused")).toBe(params);
    expect(scopeProductTransaction("entry.set", params, null, () => "unused")).toBe(params);
  });
});

describe("transactionEnvelopesForRenderer", () => {
  const current = {
    project_root: "/project",
    generation: 7,
    batch_id: "current",
    payload: { operation: "team.mapping" },
  };
  const older = {
    project_root: "/project",
    generation: 7,
    batch_id: "older",
    payload: { operation: "project.external-refresh" },
  };

  it("keeps a caller-managed receipt on its operation-specific path", () => {
    expect(transactionEnvelopesForRenderer(
      [current],
      new Set([transactionReceiptIdentity(current)]),
    )).toEqual([]);
  });

  it("still publishes recovery, older FIFO, and raw bridge receipts", () => {
    expect(transactionEnvelopesForRenderer([current], new Set())).toEqual([current]);
    expect(transactionEnvelopesForRenderer(
      [older],
      new Set([transactionReceiptIdentity(current)]),
    )).toEqual([older]);
  });

  it("publishes a recovered durable head only once per renderer lifecycle", () => {
    expect(transactionEnvelopesForRenderer(
      [older],
      new Set(),
      new Set([transactionReceiptIdentity(older)]),
    )).toEqual([]);
    expect(transactionEnvelopesForRenderer(
      [current],
      new Set(),
      new Set([transactionReceiptIdentity(older)]),
    )).toEqual([current]);
  });
});
