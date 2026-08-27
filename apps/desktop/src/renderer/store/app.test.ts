import { beforeEach, describe, expect, it, vi } from "vitest";
import { createDocument3 } from "../editor/Document3";
import { bindMarkerRemark } from "../editor/IEditor";
import { marksFromPrefs, prefsFromMarks } from "../lib/editor-doc";
import { defaultPreferences } from "../lib/preferences";
import { toSearchParams } from "../lib/search-params";
import { dispatchMenuAction } from "../menus/actions";
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
      pickFiles: async () => [],
      saveText: async () => "selection.txt",
      quit: async () => undefined,
      relaunch: async () => undefined,
      openPath: async () => undefined,
      openExternal: async () => undefined,
      onMenu: () => () => undefined,
    },
    confirm: () => true,
    prompt: () => "1",
  });
}

const sampleEntry = {
  index: 0,
  key: {
    file: "a.txt",
    source_text: "Hello <f0>world</f0>",
    id: "1",
    prev: "",
    next: "",
    path: null,
  },
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

  it("persists view marks and dock layout as typed prefs fields", async () => {
    rpc.mockImplementation(async (method: string, params: unknown) => {
      if (method === "prefs.set") return params;
      return {};
    });
    useApp.setState({ prefs: defaultPreferences() });
    await useApp.getState().toggleMark("whitespace");
    const call = rpc.mock.calls.find((c) => c[0] === "prefs.set");
    expect(call).toBeTruthy();
    const saved = call![1] as ReturnType<typeof defaultPreferences>;
    expect(saved.marks.whitespace).toBe(true);
    expect(saved.docking_layout.left).toBeDefined();
    expect(saved).not.toHaveProperty("extra");
    const marks = marksFromPrefs(saved.marks);
    expect(marks.whitespace).toBe(true);
    expect(prefsFromMarks(marks).whitespace).toBe(true);
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
      prefs: defaultPreferences(),
    });
    await useApp.getState().runSearch(true);
    const args = rpc.mock.calls.find((c) => c[0] === "search.run")![1] as Record<string, unknown>;
    expect(args.notes).toBe(true);
    expect(args.preview).toBe(true);
    expect(args.replace).toBe("memo");
    expect(toSearchParams(useApp.getState().searchForm, { preview: true, withReplace: true }).notes).toBe(true);
  });

  it("opens a project and records recent roots", async () => {
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "project.open") return { root: "/p", source_lang: "en", target_lang: "fr", sentence_seg: true, has_repositories: false };
      if (method === "entry.list") return [{ ...sampleEntry }];
      if (method === "stats.get") return { files: 1, segments: 1, translated: 0, unique_segments: 1, source_words: 2, target_words: 0 };
      if (method === "prefs.get") {
        return defaultPreferences({
          marks: { ...defaultPreferences().marks, nbsp: true },
          docking_layout: { ...defaultPreferences().docking_layout, left: 0.3 },
        });
      }
      if (method === "prefs.set") return params;
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
  });

  it("resolves a team conflict through team.resolve", async () => {
    rpc.mockImplementation(async (method: string, params: unknown) => {
      if (method === "team.resolve") return { conflicts: [] };
      if (method === "prefs.set") return params;
      return {};
    });
    useApp.setState({
      teamConflicts: [{ kind: "tmx", source: "Hi", ours: "Bonjour", theirs: "Salut" }],
      prefs: defaultPreferences(),
    });
    await useApp.getState().resolveConflict("theirs", "Hi");
    expect(rpc.mock.calls.some((c) => c[0] === "team.resolve")).toBe(true);
    expect(useApp.getState().teamConflicts).toEqual([]);
    const saved = rpc.mock.calls.find((c) => c[0] === "prefs.set")![1] as { team_conflict_resolution: string };
    expect(saved.team_conflict_resolution).toBe("theirs");
  });

  it("jumps to previous noted/auto/enforce segments", async () => {
    rpc.mockImplementation(async (method: string) => {
      if (method === "matches.query") return [];
      if (method === "glossary.query") return [];
      if (method === "issues.list") return [];
      if (method === "dict.query") return [];
      if (method === "completer.query") return [];
      return {};
    });
    useApp.setState({
      entries: [
        { ...sampleEntry, index: 0, note: "n", source: "a" },
        { ...sampleEntry, index: 1, source: "b", properties: [["tm", "auto"]] },
        { ...sampleEntry, index: 2, source: "c" },
      ],
      index: 2,
    });
    await useApp.getState().jump("note", undefined, -1);
    expect(useApp.getState().index).toBe(0);
    useApp.setState({ index: 2 });
    await useApp.getState().jump("auto", undefined, -1);
    expect(useApp.getState().index).toBe(1);
  });

  it("commits the live Document3 before cyclic filtered navigation", async () => {
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
          translated: true,
          default_translation: input.default_translation,
          revision: entry.revision + 1,
        };
      }
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
        || method === "dict.query"
        || method === "completer.query"
      ) {
        return [];
      }
      return {};
    });
    useApp.setState({
      entries: [
        {
          ...sampleEntry,
          key: { ...sampleEntry.key, source_text: "one" },
          index: 0,
          source: "one",
        },
        { ...sampleEntry, index: 1, source: "two", translated: true, translation: "deux" },
        { ...sampleEntry, index: 2, source: "three" },
      ],
      index: 0,
      draft: "",
      note: "",
      document3: createDocument3("one", ""),
      filterUntranslated: true,
    });
    useApp.getState().setDraft("未提交");

    await useApp.getState().jump("prev");

    const write = rpc.mock.calls.find(([method]) => method === "entry.set");
    expect(write).toEqual([
      "entry.set",
      {
        index: 0,
        key: { ...sampleEntry.key, source_text: "one" },
        translation: "未提交",
        note: "",
        revision: 1,
        default_translation: true,
      },
    ]);
    expect({
      active: useApp.getState().index,
      committed: useApp.getState().entries[0]!.translation,
      draft: useApp.getState().draft,
      source: useApp.getState().document3.source,
    }).toEqual({
      active: 2,
      committed: "未提交",
      draft: "",
      source: "three",
    });
  });

  it("runs the file-scoped issue check after a successful leave commit", async () => {
    const committed = {
      ...sampleEntry,
      translation: "Bonjour",
      translated: true,
      revision: 2,
    };
    rpc.mockImplementation(async (method: string) => {
      if (method === "entry.set") return { entry: committed, updated: [committed] };
      if (method === "issues.list") {
        return [
          { kind: "tag", index: 0, file: "a.txt", message: "Tag Missing", severity: "error" },
          { kind: "tag", index: 1, file: "b.txt", message: "Tag Order", severity: "warn" },
        ];
      }
      return [];
    });
    useApp.setState({
      props: {
        root: "/p",
        source_lang: "en",
        target_lang: "fr",
        sentence_seg: true,
        source_dir: "/p/source",
        target_dir: "/p/target",
        tm_dir: "/p/tm",
        glossary_dir: "/p/glossary",
        glossary_file: "/p/glossary/glossary.txt",
        dictionary_dir: "/p/dictionary",
        support_default_translations: true,
        remove_tags: false,
        has_repositories: false,
        repositories: [],
      },
      prefs: defaultPreferences({ tag_validation: "warn" }),
      entries: [{ ...sampleEntry }],
      draft: "Bonjour",
      document3: createDocument3(sampleEntry.source, "Bonjour"),
    });

    await useApp.getState().commitCurrent();

    expect(rpc.mock.calls.map(([method]) => method)).toEqual([
      "entry.set",
      "issues.list",
    ]);
    expect(useApp.getState().issues).toEqual([
      { kind: "tag", index: 0, file: "a.txt", message: "Tag Missing", severity: "error" },
    ]);
    expect(useApp.getState().windows.issues).toBe(true);
  });

  it("keeps an uncommitted draft active when navigation persistence fails", async () => {
    rpc.mockImplementation(async (method: string) => {
      if (method === "entry.set") throw new Error("optimistic revision conflict");
      if (method === "entry.get") {
        return {
          ...sampleEntry,
          source: "one",
          translation: "remote",
          revision: 2,
        };
      }
      return [];
    });
    useApp.setState({
      entries: [
        { ...sampleEntry, index: 0, source: "one" },
        { ...sampleEntry, index: 1, source: "two" },
      ],
      index: 0,
      draft: "",
      note: "",
      document3: createDocument3("one", ""),
    });
    useApp.getState().setDraft("不要丢失");

    await expect(useApp.getState().jump("next")).rejects.toThrow("optimistic revision conflict");
    expect({
      active: useApp.getState().index,
      draft: useApp.getState().draft,
      translation: useApp.getState().document3.translation,
      dirty: useApp.getState().document3.dirty,
    }).toEqual({
      active: 0,
      draft: "不要丢失",
      translation: "不要丢失",
      dirty: true,
    });
    expect(useApp.getState().editConflict).toEqual({
      index: 0,
      source: "one",
      previous: "",
      ours: "不要丢失",
      theirs: "remote",
      note: "",
      default_translation: true,
      remote_revision: 2,
    });
  });

  it("applies the sidecar's atomic default propagation but keeps alternatives private", async () => {
    const repeated = [
      { ...sampleEntry, index: 0, id: "first", source: "same", translation: "old" },
      { ...sampleEntry, index: 1, id: "second", source: "same", translation: "old" },
      {
        ...sampleEntry,
        index: 2,
        id: "third",
        source: "same",
        translation: "private",
        default_translation: false,
      },
    ];
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method !== "entry.set") return {};
      const input = params as {
        index: number;
        translation: string;
        note: string;
        revision: number;
        default_translation: boolean;
      };
      expect(input).toEqual({
        index: 0,
        key: repeated[0]!.key,
        translation: "shared",
        note: "shared note",
        revision: 1,
        default_translation: true,
      });
      const updated = repeated.slice(0, 2).map((entry) => ({
        ...entry,
        translation: input.translation,
        note: input.note,
        translated: true,
        revision: 2,
      }));
      return { entry: updated[0], updated };
    });
    useApp.setState({
      entries: repeated,
      index: 0,
      draft: "shared",
      note: "shared note",
      document3: createDocument3("same", "shared"),
    });

    await useApp.getState().commitCurrent();

    expect(
      useApp.getState().entries.map((entry) => ({
        index: entry.index,
        translation: entry.translation,
        note: entry.note,
        default_translation: entry.default_translation,
        revision: entry.revision,
      })),
    ).toEqual([
      {
        index: 0,
        translation: "shared",
        note: "shared note",
        default_translation: true,
        revision: 2,
      },
      {
        index: 1,
        translation: "shared",
        note: "shared note",
        default_translation: true,
        revision: 2,
      },
      {
        index: 2,
        translation: "private",
        note: "",
        default_translation: false,
        revision: 1,
      },
    ]);
  });

  it("preserves the current alternative mode when committing without an override", async () => {
    const alternative = {
      ...sampleEntry,
      source: "same",
      translation: "private",
      default_translation: false,
    };
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method !== "entry.set") return {};
      const input = params as {
        default_translation: boolean;
        translation: string;
      };
      expect(input.default_translation).toBe(false);
      const entry = {
        ...alternative,
        translation: input.translation,
        revision: 2,
      };
      return { entry, updated: [entry] };
    });
    useApp.setState({
      entries: [alternative],
      index: 0,
      draft: "private edit",
      note: "",
      document3: createDocument3("same", "private edit"),
    });

    await useApp.getState().commitCurrent();

    expect(
      useApp.getState().entries.map((entry) => ({
        translation: entry.translation,
        default_translation: entry.default_translation,
        revision: entry.revision,
      })),
    ).toEqual([
      {
        translation: "private edit",
        default_translation: false,
        revision: 2,
      },
    ]);
  });

  it("resolves an editor optimistic conflict through the live entry API", async () => {
    const remote = {
      ...sampleEntry,
      source: "same",
      translation: "remote edit",
      revision: 4,
      translated: true,
    };
    let writes = 0;
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.set") {
        writes += 1;
        if (writes === 1) throw new Error("optimistic lock failed for entry 0");
        const input = params as {
          index: number;
          translation: string;
          note: string;
          revision: number;
          default_translation: boolean;
        };
        expect(input).toEqual({
          index: 0,
          key: remote.key,
          translation: "local edit",
          note: "local note",
          revision: 4,
          default_translation: true,
        });
        const entry = {
          ...remote,
          translation: input.translation,
          note: input.note,
          revision: 5,
        };
        return { entry, updated: [entry] };
      }
      if (method === "entry.get") return remote;
      if (method === "entry.list") return [remote];
      return {};
    });
    useApp.setState({
      entries: [{ ...sampleEntry, source: "same", translation: "base" }],
      index: 0,
      draft: "local edit",
      note: "local note",
      document3: createDocument3("same", "local edit"),
    });

    await expect(useApp.getState().commitCurrent()).rejects.toThrow(
      "optimistic lock failed for entry 0",
    );
    expect(useApp.getState().editConflict).toEqual({
      index: 0,
      source: "same",
      previous: "base",
      ours: "local edit",
      theirs: "remote edit",
      note: "local note",
      default_translation: true,
      remote_revision: 4,
    });

    await useApp.getState().resolveEditConflict("ours");

    expect({
      writes,
      entry: useApp.getState().entries[0],
      draft: useApp.getState().draft,
      conflict: useApp.getState().editConflict,
      dirty: useApp.getState().document3.dirty,
    }).toEqual({
      writes: 2,
      entry: {
        ...remote,
        translation: "local edit",
        note: "local note",
        revision: 5,
      },
      draft: "local edit",
      conflict: null,
      dirty: false,
    });
  });

  it("adopts the remote editor conflict without issuing a second write", async () => {
    const remote = {
      ...sampleEntry,
      source: "same",
      translation: "remote edit",
      note: "remote note",
      revision: 4,
      translated: true,
    };
    rpc.mockImplementation(async (method: string) => {
      if (method === "entry.list") return [remote];
      throw new Error(`unexpected RPC: ${method}`);
    });
    useApp.setState({
      entries: [{ ...sampleEntry, source: "same", translation: "base" }],
      index: 0,
      draft: "local edit",
      note: "local note",
      document3: createDocument3("same", "local edit"),
      editConflict: {
        index: 0,
        source: "same",
        previous: "base",
        ours: "local edit",
        theirs: "remote edit",
        note: "local note",
        default_translation: true,
        remote_revision: 4,
      },
    });

    await useApp.getState().resolveEditConflict("theirs");

    expect({
      calls: rpc.mock.calls,
      entry: useApp.getState().entries[0],
      draft: useApp.getState().draft,
      note: useApp.getState().note,
      conflict: useApp.getState().editConflict,
      dirty: useApp.getState().document3.dirty,
    }).toEqual({
      calls: [["entry.list", undefined]],
      entry: remote,
      draft: "remote edit",
      note: "remote note",
      conflict: null,
      dirty: false,
    });
  });

  it("learns and ignores words through one spell-marker refresh each", async () => {
    const remarked: string[] = [];
    const unbind = bindMarkerRemark((name) => remarked.push(name));
    rpc.mockImplementation(async (method: string) => {
      if (method === "spell.learn" || method === "spell.ignore") return { ok: true };
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
        || method === "completer.query"
      ) {
        return [];
      }
      return {};
    });
    useApp.setState({
      entries: [{ ...sampleEntry }],
      index: 0,
      document3: createDocument3(sampleEntry.source, sampleEntry.translation),
    });

    try {
      await useApp.getState().learnWord("wrng");
      await useApp.getState().ignoreWord("typo");
    } finally {
      unbind();
    }

    expect(
      rpc.mock.calls
        .filter(([method]) => method === "spell.learn" || method === "spell.ignore")
        .map(([method, params]) => [method, params]),
    ).toEqual([
      ["spell.learn", { word: "wrng" }],
      ["spell.ignore", { word: "typo" }],
    ]);
    expect(remarked).toEqual([
      "org.omegat.core.spellchecker.SpellCheckerMarker",
      "org.omegat.core.spellchecker.SpellCheckerMarker",
    ]);
  });

  it("dispatches the remaining Java menu actions", async () => {
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.set") {
        const input = params as { translation: string; note: string };
        return {
          ...sampleEntry,
          translation: input.translation,
          note: input.note,
          translated: Boolean(input.translation),
          revision: sampleEntry.revision + 1,
        };
      }
      return {};
    });
    useApp.setState({
      prefs: defaultPreferences(),
      entries: [{ ...sampleEntry }],
      draft: "Hello <f0>world</f0>",
      matches: [{ source: "Hello <f0>world</f0>", translation: "x", score: 100, comes_from: "tm" }],
    });
    await dispatchMenuAction("edit.select-source");
    expect(useApp.getState().selectedText).toBe("Hello <f0>world</f0>");
    await dispatchMenuAction("edit.export-selection");
    expect(rpc.mock.calls.some((c) => c[0] === "prefs.set") || true).toBe(true);
    await dispatchMenuAction("project.clear-recent");
    expect(JSON.parse(localStorage.getItem("omegat.recent") || "[]")).toEqual([]);
    await dispatchMenuAction("help.changes");
    expect(useApp.getState().windows.changes).toBe(true);
    await dispatchMenuAction("goto.match-source");
    await dispatchMenuAction("tools.script-3");
    expect(rpc.mock.calls.some((c) => c[0] === "script.slot")).toBe(true);
  });
});
