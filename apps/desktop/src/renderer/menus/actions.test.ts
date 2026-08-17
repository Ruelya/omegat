import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetAppState, useApp } from "../store/app";
import { DESKTOP_MENU_ACTIONS, JAVA_MENU_ACTIONS, SCRIPT_SLOT_ACTIONS, dispatchMenuAction } from "./actions";

const rpc = vi.fn();

function installBridge() {
  vi.stubGlobal("window", {
    omegat: {
      rpc,
      pickDir: async () => null,
      pickFile: async () => null,
      openPath: async () => undefined,
      openExternal: async () => undefined,
      onMenu: () => () => undefined,
    },
    confirm: () => true,
    prompt: () => "1",
  });
}

describe("menu actions", () => {
  beforeEach(() => {
    rpc.mockReset();
    installBridge();
    resetAppState();
  });

  it("lists all 120 Java MainWindowMenuHandler actions", () => {
    expect(JAVA_MENU_ACTIONS).toHaveLength(120);
    expect(new Set(JAVA_MENU_ACTIONS).size).toBe(120);
  });

  it("wires script slots 1–12", () => {
    expect(SCRIPT_SLOT_ACTIONS).toEqual(Array.from({ length: 12 }, (_, i) => `tools.script-${i + 1}`));
    expect(DESKTOP_MENU_ACTIONS).toContain("tools.script-12");
  });

  it("observable: project.edit opens the project properties dialog, not the new-project wizard", async () => {
    await dispatchMenuAction("project.edit");
    expect(useApp.getState().windows["project-edit"]).toBe(true);
    expect(useApp.getState().windows.wizard).toBeFalsy();
  });

  it("observable: project.team-new opens the team project flow", async () => {
    await dispatchMenuAction("project.team-new");
    expect(useApp.getState().windows.team).toBe(true);
    expect(useApp.getState().windows.wizard).toBeFalsy();
  });

  it("observable: edit.pdf inserts U+202C into the draft", async () => {
    useApp.setState({ draft: "ab" });
    await dispatchMenuAction("edit.pdf");
    expect(useApp.getState().draft).toContain("\u202c");
  });

  it("observable: edit.register-untranslated writes an empty translation", async () => {
    rpc.mockResolvedValue({ ok: true });
    useApp.setState({
      entries: [
        {
          index: 0,
          file: "a.txt",
          id: "1",
          source: "Hi",
          translation: "x",
          note: "",
          comment: "",
          default_translation: true,
          revision: 1,
          translated: true,
          tags: [],
          properties: [],
        },
      ],
      index: 0,
      draft: "x",
    });
    await dispatchMenuAction("edit.register-untranslated");
    expect(rpc).toHaveBeenCalled();
  });

  it("observable: each window action opens that window, not a shared wizard", async () => {
    const opens: [string, string][] = [
      ["project.new", "wizard"],
      ["project.wiki", "wiki"],
      ["project.files", "files"],
      ["edit.search", "search"],
      ["edit.replace", "replace"],
      ["edit.glossary", "glossary-add"],
      ["options.prefs", "prefs"],
      ["options.filters", "filters"],
      ["options.segmentation", "segmentation"],
      ["options.shortcuts", "shortcuts"],
      ["tools.issues", "issues"],
      ["tools.stats-standard", "stats-standard"],
      ["tools.stats-matches", "stats-matches"],
      ["tools.stats-files", "stats-files"],
      ["tools.align", "align"],
      ["tools.scripts", "scripts"],
      ["help.about", "about"],
      ["help.license", "license"],
      ["help.log", "log"],
      ["help.tip", "tip"],
      ["help.changes", "changes"],
    ];
    for (const [action, win] of opens) {
      resetAppState();
      installBridge();
      await dispatchMenuAction(action);
      expect((useApp.getState().windows as Record<string, boolean | undefined>)[win], action).toBe(true);
      if (action !== "project.new") {
        expect(useApp.getState().windows.wizard, action).toBeFalsy();
      }
    }
  });

  it("observable: bidi inserts and case change mutate the draft", async () => {
    useApp.setState({ draft: "ab" });
    await dispatchMenuAction("edit.lrm");
    expect(useApp.getState().draft).toContain("\u200e");
    await dispatchMenuAction("edit.rlm");
    expect(useApp.getState().draft).toContain("\u200f");
    await dispatchMenuAction("edit.lre");
    expect(useApp.getState().draft).toContain("\u202a");
    await dispatchMenuAction("edit.rle");
    expect(useApp.getState().draft).toContain("\u202b");
    useApp.setState({ draft: "hello" });
    await dispatchMenuAction("edit.case-upper");
    expect(useApp.getState().draft).toBe("HELLO");
    await dispatchMenuAction("edit.overwrite-source");
    expect(useApp.getState().draft).toBe("");
  });

  it("observable: view marks and completer flags persist as prefs side effects", async () => {
    rpc.mockResolvedValue({ ok: true });
    useApp.setState({ prefs: useApp.getState().prefs ?? undefined, marks: { ...useApp.getState().marks, nbsp: false } });
    const before = useApp.getState().marks.nbsp;
    await dispatchMenuAction("view.mark-nbsp");
    expect(useApp.getState().marks.nbsp).toBe(!before);
    await dispatchMenuAction("goto.notes");
    expect(useApp.getState().focusPanel).toBe("notes");
    await dispatchMenuAction("goto.editor");
    expect(useApp.getState().focusPanel).toBe("editor");
    const auto = useApp.getState().mtAutoFetch;
    await dispatchMenuAction("options.mt-auto");
    expect(useApp.getState().mtAutoFetch).toBe(!auto);
  });
});
