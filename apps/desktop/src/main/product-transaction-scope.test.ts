// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { scopeProductTransaction } from "./product-transaction-scope";

describe("scopeProductTransaction", () => {
  it("binds editor, save, and close writes to the watched project generation", () => {
    const project = { root: "/project", generation: 17 };
    let sequence = 0;
    const createId = () => `id-${++sequence}`;

    expect([
      scopeProductTransaction("entry.set", { index: 4 }, project, createId),
      scopeProductTransaction("project.save", undefined, project, createId),
      scopeProductTransaction("project.close", {}, project, createId),
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
