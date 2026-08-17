import { describe, expect, it } from "vitest";
import { DESKTOP_MENU_ACTIONS, JAVA_MENU_ACTIONS, SCRIPT_SLOT_ACTIONS } from "./actions";

describe("menu actions", () => {
  it("lists all 120 Java MainWindowMenuHandler actions", () => {
    expect(JAVA_MENU_ACTIONS).toHaveLength(120);
    expect(new Set(JAVA_MENU_ACTIONS).size).toBe(120);
    for (const required of [
      "project.import",
      "project.clear-recent",
      "project.exit",
      "project.restart",
      "edit.export-selection",
      "edit.select-source",
      "edit.multiple-default",
      "edit.multiple-alt",
      "goto.note-prev",
      "goto.auto-prev",
      "goto.enforce-prev",
      "goto.match-source",
      "help.changes",
    ]) {
      expect(JAVA_MENU_ACTIONS).toContain(required);
    }
  });

  it("wires script slots 1–12", () => {
    expect(SCRIPT_SLOT_ACTIONS).toEqual(Array.from({ length: 12 }, (_, i) => `tools.script-${i + 1}`));
    expect(DESKTOP_MENU_ACTIONS).toContain("tools.script-12");
  });
});
