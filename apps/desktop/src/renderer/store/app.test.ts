import { beforeEach, describe, expect, it, vi } from "vitest";
import { extraFromMarks, marksFromExtra } from "../lib/editor-doc";
import { serializeDockLayout } from "../lib/layout";
import { toSearchParams } from "../lib/search-params";
import { resetAppState, useApp } from "./app";

const rpc = vi.fn();

function installBridge() {
  const mem = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => {
      mem.set(k, String(v));
    },
    removeItem: (k: string) => {
      mem.delete(k);
    },
  });
  vi.stubGlobal("window", {
    omegat: {
      rpc,
      pickDir: async () => null,
      pickFile: async () => null,
      openPath: async () => undefined,
      openExternal: async () => undefined,
      onMenu: () => () => undefined,
    },
  });
}

const sampleEntry = {
  index: 0,
  file: "a.txt",
  id: "1",
  source: "Hello <f0>world</f0>",
  translation: "",
  note: "",
  comment: "",
  default_translation: true,
  revision: 1,
  translated: false,
  tags: ["<f0>", "</f0>"],
  properties: [] as [string, string][],
};

describe("app store", () => {
  beforeEach(() => {
    rpc.mockReset();
    installBridge();
    resetAppState();
  });

  it("undoes draft edits and inserts fuzzy 1–5", () => {
    useApp.setState({
      draft: "a",
      matches: [
        { source: "s", translation: "one", score: 90, comes_from: "tm" },
        { source: "s", translation: "two", score: 80, comes_from: "tm" },
      ],
    });
    useApp.getState().setDraft("ab");
    useApp.getState().undo();
    expect(useApp.getState().draft).toBe("a");
    useApp.getState().redo();
    expect(useApp.getState().draft).toBe("ab");
    useApp.getState().insertMatch(2, "overwrite");
    expect(useApp.getState().draft).toBe("two");
    useApp.getState().insertMatch(1, "insert");
    expect(useApp.getState().draft).toBe("twoone");
  });

  it("persists view marks and dock layout through prefs.set", async () => {
    rpc.mockImplementation(async (method: string, params: unknown) => {
      if (method === "prefs.set") return params;
      return {};
    });
    useApp.setState({
      prefs: {
        theme: "light",
        locale: "en",
        autosave_seconds: 180,
        fuzzy_threshold: 30,
        insert_best_match: true,
        font_ui: "IBM Plex Sans",
        font_editor: "IBM Plex Sans",
        mt_enabled: [],
        extra: {},
      },
    });
    await useApp.getState().toggleMark("whitespace");
    const call = rpc.mock.calls.find((c) => c[0] === "prefs.set");
    expect(call).toBeTruthy();
    const saved = call![1] as { extra: Record<string, string> };
    expect(saved.extra.mark_whitespace).toBe("true");
    expect(saved.extra.docking_layout).toBeTruthy();
    const marks = marksFromExtra(saved.extra);
    expect(marks.whitespace).toBe(true);
    expect(JSON.parse(saved.extra.docking_layout).left).toBeDefined();
  });

  it("builds search RPC from the Search window form", async () => {
    rpc.mockResolvedValue([]);
    useApp.setState({
      searchForm: {
        query: "note",
        replace: "memo",
        searchType: "exact",
        source: false,
        translation: false,
        notes: true,
        comments: false,
        caseSensitive: false,
        wholeWord: false,
        untranslated: false,
        author: "",
        dateFrom: "",
        dateTo: "",
      },
      prefs: {
        theme: "light",
        locale: "en",
        autosave_seconds: 180,
        fuzzy_threshold: 30,
        insert_best_match: true,
        font_ui: "IBM Plex Sans",
        font_editor: "IBM Plex Sans",
        mt_enabled: [],
        extra: {},
      },
    });
    await useApp.getState().runSearch(true);
    const args = rpc.mock.calls.find((c) => c[0] === "search.run")![1] as Record<string, unknown>;
    expect(args.notes).toBe(true);
    expect(args.preview).toBe(true);
    expect(args.replace).toBe("memo");
    expect(toSearchParams(useApp.getState().searchForm, { preview: true, withReplace: true }).notes).toBe(true);
  });

  it("opens a project and records recent roots", async () => {
    rpc.mockImplementation(async (method: string) => {
      if (method === "project.open") return { root: "/p", source_lang: "en", target_lang: "fr", sentence_seg: true, has_repositories: false };
      if (method === "entry.list") return [{ ...sampleEntry }];
      if (method === "stats.get") return { files: 1, segments: 1, translated: 0, unique_segments: 1, source_words: 2, target_words: 0 };
      if (method === "prefs.get") {
        return {
          theme: "light",
          locale: "en",
          autosave_seconds: 180,
          fuzzy_threshold: 30,
          insert_best_match: true,
          font_ui: "IBM Plex Sans",
          font_editor: "IBM Plex Sans",
          mt_enabled: [],
          extra: { docking_layout: serializeDockLayout({ ...useApp.getState().layout, left: 0.3 }), mark_nbsp: "true" },
        };
      }
      if (method === "matches.query") return [{ source: "Hello", translation: "Bonjour", score: 100, comes_from: "tm" }];
      if (method === "glossary.query") return [];
      if (method === "issues.list") return [];
      if (method === "dict.query") return [];
      if (method === "completer.query") return [];
      return {};
    });
    await useApp.getState().open("/p");
    expect(useApp.getState().screen).toBe("workspace");
    expect(useApp.getState().entries).toHaveLength(1);
    expect(useApp.getState().draft).toBe("Bonjour");
    expect(useApp.getState().marks.nbsp).toBe(true);
    expect(JSON.parse(localStorage.getItem("omegat.recent") || "[]")[0]).toBe("/p");
    expect(extraFromMarks(useApp.getState().marks).mark_nbsp).toBe("true");
  });
});
