import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { JAVA_MENU_ACTIONS } from "./menus/actions";
import { changeCase, removeDirectionChars, replaceGlossaryEntries } from "./editor/EditorUtils";
import { NotesDocument, renderGlossaryText } from "./lib/dock-models";
import {
  projectPropertiesIdentical,
  repositoryEquals,
  rootRepositoryMapping,
  setRootRepositoryMapping,
} from "./lib/project-ui";
import { SEARCH_EXPRESSION_TYPES } from "./search/SearchWindow";
import {
  bindInputShortcuts,
  bindMenuShortcuts,
  javaKeyStroke,
  mergeShortcutProperties,
} from "./lib/shortcuts";
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
    const change = load("remaining/EditorUtilsTest-testChangeCase.json");
    for (const testCase of change.cases) {
      expect(changeCase(testCase.input, "lower"), testCase.input).toBe(testCase.lower);
      expect(changeCase(testCase.input, "upper"), testCase.input).toBe(testCase.upper);
      expect(changeCase(testCase.input, "title"), testCase.input).toBe(testCase.title);
      expect(changeCase(testCase.input, "sentence"), testCase.input).toBe(testCase.sentence);
      expect(changeCase(testCase.input, "cycle"), testCase.input).toBe(testCase.cycle);
    }
    let cycled = change.cases[0].input as string;
    const cycleSequence = [];
    for (let i = 0; i < change.cycle_sequence.length; i += 1) {
      cycled = changeCase(cycled, "cycle");
      cycleSequence.push(cycled);
    }
    expect(cycleSequence).toEqual(change.cycle_sequence);
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

  it("GlossaryTextAreaTest and NotesTextAreaTest use dock product models", () => {
    const set = load("remaining/GlossaryTextAreaTest-testSetGlossaryEntries.json");
    expect(renderGlossaryText(set.entries)).toBe(set.text);
    const linked = load("remaining/GlossaryTextAreaTest-testSetGlossaryEntriesWithLink.json");
    expect(renderGlossaryText(linked.entries)).toBe(linked.text);
    const clear = load("remaining/GlossaryTextAreaTest-testClear.json");
    expect(renderGlossaryText(set.entries)).toBe(clear.before);
    expect(renderGlossaryText([])).toBe(clear.after);

    const notes = new NotesDocument();
    const note = load("remaining/NotesTextAreaTest-testSetNote.json");
    notes.set(note.set);
    expect(notes.get()).toBe(note.set);
    notes.set("");
    expect(notes.get() === null).toBe(note.empty_is_null);
    const noteClear = load("remaining/NotesTextAreaTest-testClear.json");
    notes.set(noteClear.before);
    notes.clear();
    expect(notes.get() === null).toBe(noteClear.after_is_null);
  });

  it("ProjectUICommandsTest calls repository editor product helpers", () => {
    const root = {
      repo_type: "git",
      url: "git@github.com:omegat-L10N/ja.git",
      branch: "main",
      mappings: [{ local: "/", repository: "/", includes: [], excludes: [] }],
    };
    const props = {
      root: "/tmp/project",
      source_lang: "en",
      target_lang: "ja",
      sentence_seg: true,
      export_tm_levels: "omegat level1 level2",
      has_repositories: true,
      repositories: [root],
    };
    const same = structuredClone(props);
    const identity = load("gui/ProjectUICommandsTest-testIsIdenticalOmegatProjectProperties0.json");
    expect(projectPropertiesIdentical(props, same)).toBe(identity.identical_before);
    same.export_tm_levels = "";
    expect(projectPropertiesIdentical(props, same)).toBe(identity.identical_after_export_levels_change);

    const found = rootRepositoryMapping([root]);
    const get = load("gui/ProjectUICommandsTest-testGetRootRepositoryMapping0.json");
    expect({
      branch: found?.branch,
      type: found?.repo_type,
      url: found?.url,
      mapping_count: found?.mappings.length,
      local: found?.mappings[0]?.local,
      repository: found?.mappings[0]?.repository,
    }).toEqual({
      branch: get.branch,
      type: get.type,
      url: get.url,
      mapping_count: get.mapping_count,
      local: get.local,
      repository: get.repository,
    });

    const svn = { ...root, repo_type: "svn" };
    const svnGolden = load("gui/ProjectUICommandsTest-testGetRootRepositoryMappingSvn.json");
    const svnRoot = rootRepositoryMapping([svn]);
    expect({
      type: svnRoot?.repo_type,
      mapping_count: svnRoot?.mappings.length,
      local: svnRoot?.mappings[0]?.local,
      repository: svnRoot?.mappings[0]?.repository,
    }).toEqual({
      type: svnGolden.type,
      mapping_count: svnGolden.mapping_count,
      local: svnGolden.local,
      repository: svnGolden.repository,
    });

    const replacement = { ...root, url: "https://github.com/omegat-L10N/ja.git" };
    const updated = setRootRepositoryMapping([root], replacement);
    const set = load("gui/ProjectUICommandsTest-testSetRootRepositoryMapping0.json");
    expect({
      repository_count: updated.length,
      type: updated[0]?.repo_type,
      branch: updated[0]?.branch,
      url: updated[0]?.url,
      local: updated[0]?.mappings[0]?.local,
      repository: updated[0]?.mappings[0]?.repository,
    }).toEqual({
      repository_count: set.repository_count,
      type: set.type,
      branch: set.branch,
      url: set.url,
      local: set.local,
      repository: set.repository,
    });

    const equality = load("gui/ProjectUICommandsTest-testIsRepositoryEqual.json");
    expect(repositoryEquals(root, replacement)).toBe(equality.different_url);
    expect(repositoryEquals(root, root)).toBe(equality.same_object);
    expect(repositoryEquals(root, { ...root, mappings: [] })).toBe(equality.mappings_ignored);
  });
});

describe("leftover editor / align / finder / mt / cli goldens", () => {
  it("BiDi / Whitespace / ComesFrom markers disabled goldens assert_eq", async () => {
    const { BidiMarkers } = await import("./editor/mark/BidiMarkers");
    const { WhitespaceMarker } = await import("./editor/mark/WhitespaceMarker");
    const { ComesFromAutoTMMarker } = await import("./editor/mark/ComesFromAutoTMMarker");
    const { ComesFromMTMarker } = await import("./editor/mark/ComesFromMTMarker");
    const bidiOff = load("editor/BiDiMarkersTest#testBidiMarkersDisabled.json");
    const bidi = new BidiMarkers();
    bidi.enabled = bidiOff.enabled;
    expect(
      bidi.getMarksForEntry({
        sourceText: bidiOff.source,
        translationText: bidiOff.translation,
        isActive: bidiOff.is_active,
      }),
    ).toBeNull();
    const wsOff = load("editor/WhitespaceMarkerTest#testMarkersDisabled.json");
    const ws = new WhitespaceMarker();
    ws.enabled = wsOff.enabled ?? false;
    expect(
      ws.getMarksForEntry({
        sourceText: wsOff.source ?? "source",
        translationText: wsOff.translation ?? null,
        isActive: true,
      }),
    ).toBeNull();
    const autoOff = load("editor/ComesFromAutoTMMarkerTest#testMarkersDisabled.json");
    const auto = new ComesFromAutoTMMarker();
    auto.markAutoPopulated = false;
    expect(auto.getMarksForEntry({ sourceText: null, translationText: null, isActive: true })).toBeNull();
    expect(autoOff.marks).toBeNull();
    const mtOff = load("editor/ComesFromMTMarkerTest#testMarkersDisabled.json");
    const mt = new ComesFromMTMarker();
    expect(mt.getMarksForEntry({ sourceText: "source", translationText: "target", isActive: false })).toBeNull();
    expect(mtOff.marks).toBeNull();
  });

  it("SearchWindowTest modes and radio types assert_eq", () => {
    const search = load("gui/SearchWindowTest-testLoadSearchWindow.json");
    const replace = load("gui/SearchWindowTest-testLoadSearchAndReplaceWindow.json");
    expect(search.mode).toBe("search");
    expect(replace.mode).toBe("replace");
    const types = load("gui/SearchWindowTest-testSearchTypeFollowsTheSelectedRadioButton.json");
    const replaceTypes = load("gui/SearchWindowTest-testReplaceTypeFollowsTheSelectedRadioButton.json");
    expect(SEARCH_EXPRESSION_TYPES).toEqual(types.types);
    expect(SEARCH_EXPRESSION_TYPES).toEqual(replaceTypes.types);
  });

  it("PropertiesShortcutsTest methods assert_eq", () => {
    const get = load("remaining/PropertiesShortcutsTest-testGetKeyStroke.json");
    const properties = mergeShortcutProperties(get.defaults_text, get.user_text);
    for (const [action, stroke] of Object.entries(get.strokes)) {
      expect(javaKeyStroke(properties, action), action).toBe(stroke);
    }
    let errorName = "";
    try {
      javaKeyStroke(properties, "OUT_OF_LIST");
    } catch (error) {
      errorName = (error as Error).name;
    }
    expect(errorName).toBe(get.missing_error);

    const menu = load("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuBar.json");
    const bound = bindMenuShortcuts(
      [
        {
          children: [
            {
              children: [
                { action: "TEST_USER_1" },
                { action: "OUT_OF_LIST", accelerator: "ctrl pressed X" },
              ],
            },
            { action: "TEST_DELETE", accelerator: "ctrl pressed D" },
          ],
        },
      ],
      properties,
    );
    expect({
      parent: bound[0]!.accelerator ?? null,
      child: bound[0]!.children![0]!.accelerator ?? null,
      delete: bound[0]!.children![1]!.accelerator ?? null,
      user: bound[0]!.children![0]!.children![0]!.accelerator ?? null,
      unknown: bound[0]!.children![0]!.children![1]!.accelerator ?? null,
    }).toEqual(menu.accelerators);

    const item = load("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuItem.json");
    expect([
      bindMenuShortcuts([{ action: "TEST_SAVE" }], properties)[0]!.accelerator,
      bindMenuShortcuts([{ action: "TEST_DELETE", accelerator: "ctrl pressed D" }], properties)[0]!.accelerator,
      bindMenuShortcuts([{ action: "OUT_OF_LIST", accelerator: "ctrl pressed D" }], properties)[0]!.accelerator,
    ]).toEqual(item.accelerators);

    const recursive = load("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuItemRecursive.json");
    expect(recursive.accelerators).toEqual(menu.accelerators);

    const input = load("remaining/PropertiesShortcutsTest-testBindKeyStrokesInputMapObjectArr.json");
    const inputMap = bindInputShortcuts(
      { "ctrl pressed D": "TEST_DELETE" },
      properties,
      ["TEST_SAVE", "TEST_CUT", "TEST_USER_1", "TEST_DELETE"],
    );
    const bindings = Object.entries(inputMap)
      .map(([stroke, action]) => ({ stroke, action }))
      .sort((a, b) => a.stroke.localeCompare(b.stroke));
    expect(bindings).toEqual(input.bindings);
    expect(Object.keys(inputMap)).toHaveLength(input.size);

    const bundled = load("remaining/PropertiesShortcutsTest-testLoadBundled.json");
    const selected = Object.fromEntries(
      Object.keys(bundled.strokes).map((action) => [action, javaKeyStroke(properties, action)]),
    );
    expect(selected).toEqual(bundled.strokes);
  });
});
