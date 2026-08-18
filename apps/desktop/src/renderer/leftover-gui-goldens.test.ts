import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { JAVA_MENU_ACTIONS } from "./menus/actions";
import { changeCase, removeDirectionChars, replaceGlossaryEntries } from "./editor/EditorUtils";
import type { WindowId } from "./lib/types";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../fixtures/goldens");

function load(rel: string) {
  const v = JSON.parse(readFileSync(join(goldDir, rel), "utf8"));
  expect(v.exported_by).toBe("org.omegat.tools.ExportGoldens");
  expect(String(v.java_test)).toContain("#");
  return v;
}

/** Java DialogsTest window id → desktop `WindowId` (product path). */
const DIALOG_WINDOWS: Record<string, WindowId> = {
  about: "about",
  license: "license",
  log: "log",
  changes: "changes",
  "project-edit": "project-edit",
  "glossary-new": "glossary-add",
  "project-new": "wizard",
  "team-new": "team",
  "goto-segment": "search",
  "filename-patterns": "filters",
  "file-collision": "team",
};

describe("leftover GUI Java *Test goldens", () => {
  it("MainWindowMenuTest#testMenuActions assert_eq 120", () => {
    const g = load("gui/MainWindowMenuTest-testMenuActions.json");
    expect(JAVA_MENU_ACTIONS).toHaveLength(g.action_count);
  });

  it("DialogsTest windows construct on the desktop path", () => {
    const files = [
      "gui/DialogsTest-testAboutDialog.json",
      "gui/DialogsTest-testCreateGlossaryEntryDialog.json",
      "gui/DialogsTest-testFileCollisionDialog.json",
      "gui/DialogsTest-testFilenamePatternsEditor.json",
      "gui/DialogsTest-testGoToSegmentDialog.json",
      "gui/DialogsTest-testLastChangesDialog.json",
      "gui/DialogsTest-testLicenseDialog.json",
      "gui/DialogsTest-testLogDialog.json",
      "gui/DialogsTest-testNewProjectFileChooser.json",
      "gui/DialogsTest-testNewTeamProject.json",
      "gui/DialogsTest-testProjectPropertiesDialog.json",
    ];
    for (const rel of files) {
      const g = load(rel);
      expect(g.constructs).toBe(true);
      const desktop = DIALOG_WINDOWS[g.window as string];
      expect(desktop, g.window).toBeTruthy();
    }
  });

  it("EditorUtilsTest remaining goldens assert_eq", () => {
    const dir = load("remaining/EditorUtilsTest-testRemoveDirectionChars.json");
    for (const c of dir.cases) {
      expect(removeDirectionChars(c.input)).toBe(c.output);
    }
    const cases = load("remaining/EditorUtilsTest-testChangeCase.json");
    expect(changeCase(cases.input, "lower")).toBe(cases.lower);
    expect(changeCase(cases.input, "upper")).toBe(cases.upper);
    expect(changeCase(cases.input, "title")).toBe(cases.title);
    expect(changeCase(cases.input, "sentence")).toBe(cases.sentence);
  });

  it("EditorUtilsTest#testReplaceGlossaryEntries assert_eq", () => {
    const g = load("remaining/EditorUtilsTest-testReplaceGlossaryEntries.json");
    const entries = [
      { source: "snowman", target: "sneeuwpop" },
      { source: "Bob", target: "Blub" },
    ];
    expect(replaceGlossaryEntries(g.src, entries)).toBe(g.out);
    const multi = [{ source: "snowman party", target: "sneeuwpop parti" }, ...entries];
    expect(replaceGlossaryEntries(g.multi_src, multi)).toBe(g.multi_out);
    expect(replaceGlossaryEntries(g.final_src, multi)).toBe(g.final_out);
  });
});
