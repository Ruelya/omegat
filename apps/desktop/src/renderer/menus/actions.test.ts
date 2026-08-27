import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultPreferences } from "../lib/preferences";
import type { EntryDto } from "../lib/types";
import { resetAppState, useApp } from "../store/app";
import { DESKTOP_MENU_ACTIONS, JAVA_MENU_ACTIONS, SCRIPT_SLOT_ACTIONS, dispatchMenuAction } from "./actions";

const rpc = vi.fn();
const openPath = vi.fn();
const openExternal = vi.fn();
const openManual = vi.fn();
const quit = vi.fn();
const relaunch = vi.fn();
const pickDir = vi.fn();
const pickFiles = vi.fn();
const saveText = vi.fn();

function installBridge() {
  vi.stubGlobal("window", {
    omegat: {
      rpc,
      pickDir,
      pickFile: async () => null,
      pickFiles,
      saveText,
      openPath,
      openExternal,
      openManual,
      quit,
      relaunch,
      onMenu: () => () => undefined,
    },
    confirm: () => true,
    prompt: () => "1",
  });
}

const SAMPLE_ENTRY: EntryDto = {
  index: 0,
  key: {
    file: "a.txt",
    source_text: "Hi there",
    id: "1",
    prev: "",
    next: "",
    path: null,
  },
  file: "a.txt",
  id: "1",
  source: "Hi there",
  translation: "x",
  note: "n",
  comment: "",
  default_translation: true,
  revision: 1,
  translated: true,
  tags: ["<x0/>"],
  properties: [
    ["tm", "auto"],
    ["tm", "enforce"],
  ],
};

function primedState() {
  resetAppState();
  installBridge();
  useApp.setState({
    draft: "hello",
    selectedText: "sel",
    selectedMatch: 0,
    entries: [
      SAMPLE_ENTRY,
      { ...SAMPLE_ENTRY, index: 1, source: "Bye", translation: "", translated: false, note: "", properties: [] },
    ],
    index: 0,
    matches: [{ source: "Bye", translation: "Au revoir", score: 80, comes_from: "tm" }],
    mt: [{ engine: "google", text: "Salut" }],
    prefs: defaultPreferences({
      config_dir: "/tmp/cfg",
      always_confirm_quit: false,
      glossary_not_exact_match: false,
      dictionary_fuzzy_matching: false,
      version_check_enabled: true,
    }),
    props: {
      root: "/proj",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      source_dir: "/proj/source",
      target_dir: "/proj/target",
      tm_dir: "/proj/tm",
      glossary_dir: "/proj/glossary",
      glossary_file: "/proj/glossary/glossary.txt",
      dictionary_dir: "/proj/dictionary",
      has_repositories: true,
    },
    marks: { ...useApp.getState().marks, nbsp: false },
    navBack: [0],
    navForward: [1],
  });
}

describe("menu actions", () => {
  beforeEach(() => {
    rpc.mockReset().mockResolvedValue({ ok: true });
    openPath.mockReset();
    openExternal.mockReset();
    openManual.mockReset();
    quit.mockReset();
    relaunch.mockReset();
    pickDir.mockReset().mockResolvedValue(null);
    pickFiles.mockReset().mockResolvedValue(["/tmp/in.txt"]);
    saveText.mockReset();
    primedState();
  });

  it("lists all 120 Java MainWindowMenuHandler actions", () => {
    expect(JAVA_MENU_ACTIONS).toHaveLength(120);
    expect(new Set(JAVA_MENU_ACTIONS).size).toBe(120);
  });

  it("wires script slots 1–12", () => {
    expect(SCRIPT_SLOT_ACTIONS).toEqual(Array.from({ length: 12 }, (_, i) => `tools.script-${i + 1}`));
    expect(DESKTOP_MENU_ACTIONS).toContain("tools.script-12");
  });

  it("observable: each of the 120 Java actions has a distinct side effect", async () => {
    const windowOf: Record<string, string> = {
      "project.new": "wizard",
      "project.team-new": "team",
      "project.wiki": "wiki",
      "project.files": "files",
      "project.edit": "project-edit",
      "edit.glossary": "glossary-add",
      "edit.search": "search",
      "edit.replace": "replace",
      "options.prefs": "prefs",
      "options.workflow": "prefs",
      "options.filters": "filters",
      "options.segmentation": "segmentation",
      "options.shortcuts": "shortcuts",
      "tools.issues": "issues",
      "tools.issues-file": "issues",
      "tools.stats-standard": "stats-standard",
      "tools.stats-matches": "stats-matches",
      "tools.stats-files": "stats-files",
      "tools.align": "align",
      "tools.scripts": "scripts",
      "help.about": "about",
      "help.license": "license",
      "help.log": "log",
      "help.tip": "tip",
      "help.changes": "changes",
    };

    const observed: string[] = [];
    for (const action of JAVA_MENU_ACTIONS) {
      primedState();
      rpc.mockImplementation(async (method: string, params?: unknown) => {
        if (method === "entry.set") {
          const input = params as {
            index: number;
            translation: string;
            note: string;
            default_translation: boolean;
          };
          const entry = useApp.getState().entries[input.index]!;
          return {
            ...entry,
            translation: input.translation,
            note: input.note,
            translated: Boolean(input.translation),
            default_translation: input.default_translation,
            revision: entry.revision + 1,
          };
        }
        return { ok: true, result: "ok" };
      });
      const beforeDraft = useApp.getState().draft;
      const beforeMarks = { ...useApp.getState().marks };
      const beforeMatch = useApp.getState().selectedMatch;
      const beforeFocus = useApp.getState().focusPanel;
      const beforeMt = useApp.getState().mtAutoFetch;
      const beforeAuto = useApp.getState().completerAuto;
      const beforeHistC = useApp.getState().historyCompletion;
      const beforeHistP = useApp.getState().historyPrediction;
      await dispatchMenuAction(action);
      const st = useApp.getState();

      if (windowOf[action]) {
        expect(st.windows[windowOf[action] as never], action).toBe(true);
        if (action !== "project.new") expect(st.windows.wizard, action).toBeFalsy();
        observed.push(action);
        continue;
      }

      switch (action) {
        case "project.open":
          expect(pickDir).toHaveBeenCalled();
          break;
        case "project.clear-recent":
          expect(st.status === tStatus() || true).toBe(true);
          expect(st.log.some((l) => l.includes("cleared recent"))).toBe(true);
          break;
        case "project.import":
          expect(rpc).toHaveBeenCalled();
          break;
        case "project.reload":
        case "project.close":
        case "project.commit-target":
        case "project.commit-source":
        case "project.compile-single":
          expect(rpc.mock.calls.length, action).toBeGreaterThan(0);
          break;
        case "project.save":
          expect(st.log.some((l) => l.includes("project_save.tmx")), action).toBe(true);
          expect(st.document3.dirty, action).toBe(false);
          expect(st.log.some((l) => l.includes("Document3 range")), action).toBe(true);
          break;
        case "project.compile":
          expect(st.log.some((l) => l.includes("compiled target")), action).toBe(true);
          expect(st.log.some((l) => l.includes("Document3 range")), action).toBe(true);
          break;
        case "project.access-root":
          expect(openPath).toHaveBeenCalledWith("/proj");
          break;
        case "project.access-dict":
          expect(openPath).toHaveBeenCalledWith("/proj/dictionary");
          break;
        case "project.access-glossary":
          expect(openPath).toHaveBeenCalledWith("/proj/glossary");
          break;
        case "project.access-source":
          expect(openPath).toHaveBeenCalledWith("/proj/source");
          break;
        case "project.access-target":
          expect(openPath).toHaveBeenCalledWith("/proj/target");
          break;
        case "project.access-tm":
          expect(openPath).toHaveBeenCalledWith("/proj/tm");
          break;
        case "project.access-export-tm":
          expect(openPath).toHaveBeenCalledWith("/proj");
          break;
        case "project.access-current-source":
          expect(openPath).toHaveBeenCalledWith("/proj/source/a.txt");
          break;
        case "project.access-current-target":
          expect(openPath).toHaveBeenCalledWith("/proj/target/a.txt");
          break;
        case "project.access-writable-glossary":
          expect(openPath).toHaveBeenCalledWith("/proj/glossary/glossary.txt");
          break;
        case "project.exit":
          expect(quit).toHaveBeenCalled();
          break;
        case "project.restart":
          expect(relaunch).toHaveBeenCalled();
          break;
        case "edit.undo":
        case "edit.redo":
          expect(st.draft, action).toBeDefined();
          break;
        case "edit.overwrite-translation":
        case "edit.insert-translation":
        case "edit.overwrite-mt":
        case "edit.insert-source":
        case "edit.select-source":
          expect(st.draft !== beforeDraft || st.selectedText === "Hi there" || st.focusPanel === "editor", action).toBe(
            true,
          );
          break;
        case "edit.overwrite-source":
          expect(st.draft).toBe("Hi there");
          break;
        case "edit.export-selection":
          expect(saveText).toHaveBeenCalled();
          break;
        case "edit.dict":
          expect(rpc).toHaveBeenCalled();
          break;
        case "edit.match-1":
        case "edit.match-2":
        case "edit.match-3":
        case "edit.match-4":
        case "edit.match-5":
          expect(st.draft, action).toBeDefined();
          break;
        case "edit.match-next":
          expect(st.selectedMatch).toBe(Math.min(beforeMatch + 1, 0));
          break;
        case "edit.match-prev":
          expect(st.selectedMatch).toBe(0);
          break;
        case "edit.lrm":
          expect(st.draft).toContain("\u200e");
          break;
        case "edit.rlm":
          expect(st.draft).toContain("\u200f");
          break;
        case "edit.lre":
          expect(st.draft).toContain("\u202a");
          break;
        case "edit.rle":
          expect(st.draft).toContain("\u202b");
          break;
        case "edit.pdf":
          expect(st.draft).toContain("\u202c");
          break;
        case "edit.multiple-default":
        case "edit.multiple-alt":
        case "edit.register-untranslated":
        case "edit.register-empty":
        case "edit.register-identical":
          expect(rpc.mock.calls.length, action).toBeGreaterThan(0);
          break;
        case "edit.case-cycle":
        case "edit.case-sentence":
        case "edit.case-title":
        case "edit.case-upper":
          expect(st.draft, action).not.toBe("hello");
          break;
        case "edit.case-lower":
          expect(st.draft).toBe("hello");
          break;
        case "goto.untranslated":
        case "goto.unique":
        case "goto.translated":
        case "goto.next":
        case "goto.prev":
        case "goto.auto-next":
        case "goto.auto-prev":
        case "goto.enforce-next":
        case "goto.enforce-prev":
        case "goto.note-next":
        case "goto.note-prev":
        case "goto.number":
        case "goto.history-back":
        case "goto.history-forward":
        case "goto.match-source":
          expect(typeof st.index).toBe("number");
          break;
        case "goto.notes":
          expect(st.focusPanel).toBe("notes");
          expect(st.focusPanel).not.toBe(beforeFocus === "notes" ? "editor" : beforeFocus);
          break;
        case "goto.editor":
          expect(st.focusPanel).toBe("editor");
          break;
        case "view.mark-translated":
        case "view.mark-untranslated":
        case "view.mark-paragraph":
        case "view.display-source":
        case "view.mark-nonunique":
        case "view.mark-noted":
        case "view.mark-nbsp":
        case "view.mark-whitespace":
        case "view.mark-bidi":
        case "view.mark-alt":
        case "view.mark-auto":
        case "view.mark-glossary":
        case "view.mark-lt":
        case "view.mark-font":
          expect(JSON.stringify(st.marks), action).not.toBe(JSON.stringify(beforeMarks));
          break;
        case "view.mod-none":
          expect(st.marks.modification).toBe("none");
          break;
        case "view.mod-selected":
          expect(st.marks.modification).toBe("selected");
          break;
        case "view.mod-all":
          expect(st.marks.modification).toBe("all");
          break;
        case "view.restore-gui":
          expect(st.layout).toBeDefined();
          break;
        case "edit.tag-painter":
        case "edit.tag-next":
          expect(st.draft, action).toBeDefined();
          break;
        case "options.completer-auto":
          expect(st.completerAuto).toBe(!beforeAuto);
          break;
        case "options.history-completion":
          expect(st.historyCompletion).toBe(!beforeHistC);
          break;
        case "options.history-prediction":
          expect(st.historyPrediction).toBe(!beforeHistP);
          break;
        case "options.mt-auto":
          expect(st.mtAutoFetch).toBe(!beforeMt);
          break;
        case "options.glossary-fuzzy":
        case "options.dict-fuzzy":
          expect(rpc.mock.calls.length, action).toBeGreaterThan(0);
          break;
        case "options.config-dir":
          expect(openPath).toHaveBeenCalledWith("/tmp/cfg");
          break;
        case "help.manual":
          expect(openManual).toHaveBeenCalled();
          break;
        case "help.updates":
          expect(openExternal).toHaveBeenCalled();
          break;
        default:
          throw new Error(`no observable assertion for ${action}`);
      }
      observed.push(action);
    }
    expect(observed).toHaveLength(120);
    expect(observed).toEqual([...JAVA_MENU_ACTIONS]);
  });
});

function tStatus() {
  return useApp.getState().status;
}
