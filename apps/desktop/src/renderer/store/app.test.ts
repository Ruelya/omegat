import { beforeEach, describe, expect, it, vi } from "vitest";
import { createDocument3 } from "../editor/Document3";
import { bindMarkerRemark, IEditor } from "../editor/IEditor";
import { marksFromPrefs, prefsFromMarks } from "../lib/editor-doc";
import { defaultPreferences } from "../lib/preferences";
import { projectEvents } from "../lib/project-events";
import { toSearchParams } from "../lib/search-params";
import { dispatchMenuAction } from "../menus/actions";
import type { RpcOperationEvent } from "../../shared/rpc-operation";
import {
  connectExternalProjectEvents,
  connectRpcOperationEvents,
  resetAppState,
  useApp,
} from "./app";

const rpc = vi.fn();
const cancelRpc = vi.fn(async (_requestId: string) => true);
let rpcOperationListener: ((event: RpcOperationEvent) => void) | null = null;

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
      cancelRpc,
      onRpcOperation: (listener: (event: RpcOperationEvent) => void) => {
        rpcOperationListener = listener;
        return () => {
          if (rpcOperationListener === listener) rpcOperationListener = null;
        };
      },
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
    cancelRpc.mockClear();
    rpcOperationListener = null;
    installBridge();
    resetAppState();
  });

  it("undoes draft edits and inserts fuzzy 1–5", () => {
    useApp.setState({
      document3: createDocument3("", "a"),
      matches: [
        { source: "s", translation: "one", score: 90, comes_from: "tm" },
        { source: "s", translation: "two", score: 80, comes_from: "tm" },
      ],
    });
    useApp.getState().setDraft("ab");
    useApp.getState().undo();
    expect(useApp.getState().document3.translation).toBe("a");
    useApp.getState().redo();
    expect(useApp.getState().document3.translation).toBe("ab");
    useApp.getState().insertMatch(2, "overwrite");
    expect(useApp.getState().document3.translation).toBe("two");
    useApp.getState().insertMatch(1, "insert");
    expect(useApp.getState().document3.translation).toBe("twoone");
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
    expect(useApp.getState().document3.translation).toBe("Bonjour");
    expect(useApp.getState().marks.nbsp).toBe(true);
    expect(JSON.parse(localStorage.getItem("omegat.recent") || "[]")[0]).toBe("/p");
  });

  it("routes IEditor selection commands through Document3 without advancing the entry", async () => {
    const props = {
      root: "/p",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      has_repositories: false,
    };
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.set") {
        const input = params as { translation: string; revision: number };
        const updated = {
          ...useApp.getState().entries[0]!,
          translation: input.translation,
          revision: input.revision + 1,
          translated: input.translation.length > 0,
        };
        return { entry: updated, updated: [updated] };
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    const entry = {
      ...sampleEntry,
      translation: "alpha beta",
      translated: true,
    };
    useApp.setState({
      props,
      screen: "workspace",
      entries: [entry],
      index: 0,
      document3: createDocument3(entry.source, entry.translation),
      editorSelection: { anchor: 6, focus: 10 },
      completer: [{ kind: "history", text: "beta", detail: "" }],
      prefs: defaultPreferences({ tag_validation: "none" }),
    });

    IEditor.insertText("X");
    expect({
      draft: useApp.getState().document3.translation,
      selected: IEditor.getSelectedText(),
      position: IEditor.getCurrentPositionInEntryTranslationInEditor(),
    }).toEqual({
      draft: "alpha X",
      selected: "",
      position: { position: 7 },
    });

    useApp.getState().setEditorSelection({ anchor: 5, focus: 0 });
    expect(IEditor.getSelectedText()).toBe("alpha");
    expect(IEditor.getCurrentPositionInEntryTranslationInEditor()).toEqual({
      selectionStart: 0,
      selectionEnd: 5,
    });
    IEditor.changeCase("upper");
    expect(useApp.getState().document3.translation).toBe("ALPHA X");

    await IEditor.commitAndDeactivate();
    expect({
      index: useApp.getState().index,
      editMode: useApp.getState().document3.editMode,
      position: IEditor.getCurrentPositionInEntryTranslationInEditor(),
      completer: useApp.getState().completer,
    }).toEqual({
      index: 0,
      editMode: false,
      position: { position: -1 },
      completer: [],
    });

    IEditor.activateEntry();
    useApp.getState().setEditorSelection({ anchor: 2, focus: 2 });
    await IEditor.commitAndLeave();
    expect({
      index: useApp.getState().index,
      editMode: useApp.getState().document3.editMode,
      selection: useApp.getState().editorSelection,
      calls: rpc.mock.calls.map(([method]) => method),
    }).toEqual({
      index: 0,
      editMode: true,
      selection: { anchor: 2, focus: 2 },
      calls: ["entry.set", "entry.set"],
    });
  });

  it("cancels an older dock load when a newer segment selection wins", async () => {
    let resolveOldMatches!: (value: unknown[]) => void;
    const oldMatches = new Promise<unknown[]>((resolve) => {
      resolveOldMatches = resolve;
    });
    rpc.mockImplementation(async (method: string, params?: { index?: number }) => {
      if (method === "matches.query" && params?.index === 0) return oldMatches;
      if (method === "matches.query") {
        return [{ source: "second", translation: "new result", score: 99, comes_from: "tm/new" }];
      }
      if (
        method === "glossary.query"
        || method === "issues.list"
        || method === "dict.query"
        || method === "completer.query"
      ) {
        return [];
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    const secondEntry = {
      ...sampleEntry,
      index: 1,
      key: {
        ...sampleEntry.key,
        source_text: "second",
        id: "2",
        path: "/second",
      },
      id: "2",
      source: "second",
      translation: "deux",
      translated: true,
    };
    useApp.setState({
      entries: [{ ...sampleEntry }, secondEntry],
      index: 0,
      document3: createDocument3(sampleEntry.source, ""),
      note: "",
      prefs: defaultPreferences({ insert_best_match: false }),
    });

    const older = useApp.getState().select(0, false);
    const newer = useApp.getState().select(1, false);
    await newer;
    resolveOldMatches([{ source: "first", translation: "stale", score: 100, comes_from: "tm/old" }]);
    await older;

    expect({
      index: useApp.getState().index,
      draft: useApp.getState().document3.translation,
      matches: useApp.getState().matches,
      staleGlossaryRequest: rpc.mock.calls.some(
        ([method, params]) => method === "glossary.query" && params?.index === 0,
      ),
    }).toEqual({
      index: 1,
      draft: "deux",
      matches: [{ source: "second", translation: "new result", score: 99, comes_from: "tm/new" }],
      staleGlossaryRequest: false,
    });
    expect(cancelRpc).toHaveBeenCalledTimes(1);
  });

  it("cancels same-key dock work across project switch and close events", async () => {
    let activeRoot = "";
    let resolveOldMt!: (value: { engine: string; text: string }) => void;
    let resolveOldDict!: (value: unknown[]) => void;
    const oldMt = new Promise<{ engine: string; text: string }>((resolve) => {
      resolveOldMt = resolve;
    });
    const oldDict = new Promise<unknown[]>((resolve) => {
      resolveOldDict = resolve;
    });
    rpc.mockImplementation(async (
      method: string,
      params?: { root?: string; engine?: string; word?: string },
    ) => {
      if (method === "project.open") {
        activeRoot = params?.root ?? "";
        return {
          root: activeRoot,
          source_lang: "en",
          target_lang: "fr",
          sentence_seg: true,
          has_repositories: false,
        };
      }
      if (method === "entry.list") {
        return [{
          ...sampleEntry,
          translation: activeRoot === "/second" ? "second project" : "first project",
          translated: true,
        }];
      }
      if (method === "stats.get") {
        return { files: 1, segments: 1, translated: 1, unique_segments: 1, source_words: 2, target_words: 2 };
      }
      if (method === "prefs.get") {
        return defaultPreferences({
          insert_best_match: false,
          dictionary_auto_search: false,
          mt_auto_fetch: false,
        });
      }
      if (method === "prefs.set") return params;
      if (method === "matches.query" || method === "glossary.query" || method === "issues.list") return [];
      if (method === "completer.query") return [];
      if (method === "mt.query" && params?.engine === "slow") return oldMt;
      if (method === "dict.query" && params?.word === "slow") return oldDict;
      if (method === "project.close") return { ok: true };
      throw new Error(`unexpected RPC: ${method}`);
    });

    await useApp.getState().open("/first");
    const staleMt = useApp.getState().queryMt("slow");
    await useApp.getState().open("/second");
    resolveOldMt({ engine: "slow", text: "stale first-project MT" });
    await staleMt;
    expect({
      root: useApp.getState().props?.root,
      draft: useApp.getState().document3.translation,
      mt: useApp.getState().mt,
    }).toEqual({
      root: "/second",
      draft: "second project",
      mt: [],
    });

    const staleDict = useApp.getState().queryDict("slow");
    await useApp.getState().closeProject();
    resolveOldDict([{ word: "stale", definition: "old project", source: "old.dsl" }]);
    await staleDict;
    expect({
      screen: useApp.getState().screen,
      props: useApp.getState().props,
      entries: useApp.getState().entries,
      matches: useApp.getState().matches,
      glossary: useApp.getState().glossary,
      mt: useApp.getState().mt,
      dict: useApp.getState().dict,
      completer: useApp.getState().completer,
    }).toEqual({
      screen: "welcome",
      props: null,
      entries: [],
      matches: [],
      glossary: [],
      mt: [],
      dict: [],
      completer: [],
    });
    expect(cancelRpc.mock.calls.map(([requestId]) => requestId)).toHaveLength(2);
    expect(cancelRpc.mock.calls.every(([requestId]) =>
      /^renderer-\d+$/.test(String(requestId))
    )).toBe(true);
  });

  it("routes compile, team, and align through explicit long-operation requests", async () => {
    rpc.mockImplementation(async (method: string) => {
      if (method === "project.compile") return { files: 2 };
      if (method === "stats.get") {
        return {
          files: 1,
          segments: 2,
          translated: 1,
          unique_segments: 2,
          source_words: 2,
          target_words: 1,
        };
      }
      if (method === "team.sync") return { action: "sync", message: "done" };
      if (method === "team.commit") return { action: "commit", message: "done" };
      if (method === "align.run") return { pairs: [] };
      throw new Error(`unexpected RPC: ${method}`);
    });

    await useApp.getState().compile();
    await useApp.getState().teamSync();
    await useApp.getState().teamCommit("target");
    await useApp.getState().runLongOperation("align", {
      source: "/source",
      target: "/target",
      dest: "/out.tmx",
    });

    expect(
      rpc.mock.calls
        .filter(([method]) => [
          "project.compile",
          "team.sync",
          "team.commit",
          "align.run",
        ].includes(method))
        .map(([method, params, requestId]) => [method, params, requestId]),
    ).toEqual([
      [
        "project.compile",
        { progress_token: "operation-compile-1" },
        "operation-compile-1",
      ],
      [
        "team.sync",
        { progress_token: "operation-teamSync-2" },
        "operation-teamSync-2",
      ],
      [
        "team.commit",
        { which: "target", progress_token: "operation-teamCommit-3" },
        "operation-teamCommit-3",
      ],
      [
        "align.run",
        {
          source: "/source",
          target: "/target",
          dest: "/out.tmx",
          progress_token: "operation-align-4",
        },
        "operation-align-4",
      ],
    ]);
    expect(useApp.getState().longOperation).toEqual({
      requestId: "operation-align-4",
      kind: "align",
      method: "align.run",
      phase: "succeeded",
      stage: null,
      error: null,
    });
  });

  it("mirrors main-process progress and cancels compile without stale publication", async () => {
    let rejectCompile!: (error: Error) => void;
    const pendingCompile = new Promise((_resolve, reject) => {
      rejectCompile = reject;
    });
    rpc.mockImplementation(async (method: string) => {
      if (method === "project.compile") return pendingCompile;
      if (method === "stats.get") throw new Error("stats must not run after cancellation");
      throw new Error(`unexpected RPC: ${method}`);
    });
    const disconnect = connectRpcOperationEvents();
    const compiling = useApp.getState().compile();
    const requestId = "operation-compile-1";
    rpcOperationListener?.({
      requestId,
      method: "project.compile",
      phase: "progress",
      stage: "compile:filters",
    });
    expect(useApp.getState().longOperation).toEqual({
      requestId,
      kind: "compile",
      method: "project.compile",
      phase: "progress",
      stage: "compile:filters",
      error: null,
    });

    cancelRpc.mockResolvedValueOnce(true);
    await expect(useApp.getState().cancelLongOperation()).resolves.toBe(true);
    expect(useApp.getState().longOperation).toEqual({
      requestId,
      kind: "compile",
      method: "project.compile",
      phase: "cancelling",
      stage: "compile:filters",
      error: null,
    });
    rpcOperationListener?.({
      requestId,
      method: "project.compile",
      phase: "cancelled",
    });
    const error = new Error("request cancelled");
    error.name = "AbortError";
    rejectCompile(error);
    await compiling;
    disconnect();

    expect(cancelRpc).toHaveBeenCalledWith(requestId);
    expect(useApp.getState().longOperation).toEqual({
      requestId,
      kind: "compile",
      method: "project.compile",
      phase: "cancelled",
      stage: "compile:filters",
      error: null,
    });
    expect(useApp.getState().status).toBe("compile cancelled");
    expect(rpc.mock.calls.map(([method]) => method)).toEqual(["project.compile"]);
  });

  it("cancels reload, keeps the rolled-back entry, and republishes its visible status", async () => {
    let rejectReload!: (error: Error) => void;
    const pendingReload = new Promise((_resolve, reject) => {
      rejectReload = reject;
    });
    const props = {
      root: "/reload-cancel",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      has_repositories: false,
    };
    rpc.mockImplementation(async (method: string) => {
      if (method === "project.save") return { ok: true };
      if (method === "project.reload") return pendingReload;
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
      ) {
        return [];
      }
      if (method === "entry.list" || method === "stats.get") {
        throw new Error(`${method} must not publish after cancellation`);
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    cancelRpc.mockImplementationOnce(async () => {
      const error = new Error("RPC request cancelled");
      error.name = "AbortError";
      rejectReload(error);
      return true;
    });
    useApp.setState({
      props,
      screen: "workspace",
      entries: [{ ...sampleEntry, translation: "kept", translated: true }],
      index: 0,
      note: "",
      document3: createDocument3(sampleEntry.source, "kept"),
      prefs: defaultPreferences({
        insert_best_match: false,
        dictionary_auto_search: false,
      }),
      completerAuto: false,
    });

    const reloading = useApp.getState().reloadProject();
    await vi.waitFor(() => {
      expect(rpc.mock.calls.some(([method]) => method === "project.reload")).toBe(true);
    });
    await expect(useApp.getState().cancelLongOperation()).resolves.toBe(true);
    await reloading;

    expect({
      status: useApp.getState().status,
      entry: useApp.getState().entries[0]?.translation,
      document: useApp.getState().document3.translation,
      operation: useApp.getState().longOperation,
      methods: rpc.mock.calls.map(([method]) => method),
    }).toEqual({
      status: "reload cancelled",
      entry: "kept",
      document: "kept",
      operation: {
        requestId: "operation-reload-1",
        kind: "reload",
        method: "project.reload",
        phase: "cancelled",
        stage: null,
        error: null,
      },
      methods: [
        "project.save",
        "project.reload",
        "matches.query",
        "glossary.query",
        "issues.list",
      ],
    });
  });

  it("treats team cancellation as a terminal UI state, not a conflict", async () => {
    let rejectSync!: (error: Error) => void;
    const pendingSync = new Promise((_resolve, reject) => {
      rejectSync = reject;
    });
    rpc.mockImplementation(async (method: string) => {
      if (method === "team.sync") return pendingSync;
      if (method === "team.conflicts") {
        throw new Error("cancelled sync must not query conflicts");
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    cancelRpc.mockImplementationOnce(async () => {
      const error = new Error("RPC request cancelled");
      error.name = "AbortError";
      rejectSync(error);
      return true;
    });

    const syncing = useApp.getState().teamSync();
    await vi.waitFor(() => {
      expect(rpc.mock.calls.some(([method]) => method === "team.sync")).toBe(true);
    });
    await useApp.getState().cancelLongOperation();
    await syncing;

    expect({
      teamMessage: useApp.getState().teamMessage,
      error: useApp.getState().error,
      operation: useApp.getState().longOperation,
      methods: rpc.mock.calls.map(([method]) => method),
    }).toEqual({
      teamMessage: "sync cancelled",
      error: null,
      operation: {
        requestId: "operation-teamSync-1",
        kind: "teamSync",
        method: "team.sync",
        phase: "cancelled",
        stage: null,
        error: null,
      },
      methods: ["team.sync"],
    });
  });

  it("commits, saves, and rebinds the complete EntryKey across project reload", async () => {
    const props = {
      root: "/p",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      has_repositories: false,
    };
    const first = {
      ...sampleEntry,
      key: {
        file: "same.txt",
        source_text: "same",
        id: "duplicate",
        prev: "",
        next: "other",
        path: "/first",
      },
      index: 0,
      file: "same.txt",
      id: "duplicate",
      source: "same",
      translation: "first",
      translated: true,
    };
    const active = {
      ...first,
      key: {
        ...first.key,
        prev: "other",
        next: "",
        path: "/second",
      },
      index: 1,
      translation: "old second",
      note: "old note",
      revision: 4,
    };
    const committed = {
      ...active,
      translation: "edited second",
      note: "edited note",
      revision: 5,
    };
    const reloadedEntries = [
      { ...committed, index: 0 },
      { ...first, index: 1 },
    ];
    const stats = {
      files: 1,
      segments: 2,
      translated: 2,
      unique_segments: 2,
      source_words: 2,
      target_words: 4,
    };
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.set") {
        expect(params).toEqual({
          index: 1,
          key: active.key,
          translation: "edited second",
          note: "edited note",
          revision: 4,
          default_translation: true,
        });
        return { entry: committed, updated: [committed] };
      }
      if (method === "project.save") return { ok: true };
      if (method === "project.reload") return { ok: true, entries: 2, props };
      if (method === "entry.list") return reloadedEntries;
      if (method === "stats.get") return stats;
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
        || method === "completer.query"
      ) {
        return [];
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    useApp.setState({
      props,
      screen: "workspace",
      entries: [first, active],
      index: 1,
      note: "edited note",
      document3: createDocument3("same", "edited second"),
      navBack: [0],
      navForward: [0],
    });

    await useApp.getState().reloadProject();

    expect(rpc.mock.calls.map(([method, params]) => [method, params])).toEqual([
      ["entry.set", {
        index: 1,
        key: active.key,
        translation: "edited second",
        note: "edited note",
        revision: 4,
        default_translation: true,
      }],
      ["issues.list", undefined],
      ["project.save", undefined],
      ["project.reload", { progress_token: "operation-reload-1" }],
      ["entry.list", undefined],
      ["stats.get", undefined],
      ["matches.query", { index: 0 }],
      ["glossary.query", { index: 0 }],
      ["issues.list", undefined],
      ["completer.query", { index: 0, prefix: "", text: "edited second" }],
    ]);
    expect(
      rpc.mock.calls.find(([method]) => method === "project.reload")?.[2],
    ).toBe("operation-reload-1");
    expect({
      index: useApp.getState().index,
      key: useApp.getState().entries[0]?.key,
      draft: useApp.getState().document3.translation,
      note: useApp.getState().note,
      source: useApp.getState().document3.source,
      translation: useApp.getState().document3.translation,
      dirty: useApp.getState().document3.dirty,
      navBack: useApp.getState().navBack,
      navForward: useApp.getState().navForward,
      stats: useApp.getState().stats,
    }).toEqual({
      index: 0,
      key: active.key,
      draft: "edited second",
      note: "edited note",
      source: "same",
      translation: "edited second",
      dirty: false,
      navBack: [],
      navForward: [],
      stats,
    });
  });

  it("adopts an external fix by complete EntryKey without committing the stale document", async () => {
    const props = {
      root: "/external",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      has_repositories: false,
    };
    const active = {
      ...sampleEntry,
      key: {
        ...sampleEntry.key,
        source_text: "same",
        prev: "before",
        next: "",
        path: "/active",
      },
      source: "same",
      translation: "before",
      note: "before note",
      translated: true,
    };
    const other = {
      ...active,
      index: 0,
      key: {
        ...active.key,
        prev: "",
        next: "after",
        path: "/other",
      },
      translation: "other",
    };
    const fixed = {
      ...active,
      index: 1,
      translation: "fixed",
      note: "fixed note",
      revision: 2,
    };
    const stats = {
      files: 1,
      segments: 2,
      translated: 2,
      unique_segments: 2,
      source_words: 2,
      target_words: 2,
    };
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.list") return [other, fixed];
      if (method === "stats.get") return stats;
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
      ) {
        return [];
      }
      throw new Error(`unexpected RPC: ${method} ${JSON.stringify(params)}`);
    });
    useApp.setState({
      props,
      screen: "workspace",
      entries: [active],
      index: 0,
      note: "stale note",
      document3: createDocument3(active.source, "stale local edit"),
      editorSelection: { anchor: 3, focus: 99 },
      prefs: defaultPreferences({
        insert_best_match: false,
        dictionary_auto_search: false,
      }),
      completerAuto: false,
    });
    const observed: Array<{
      kind: string;
      changedEntryKeys: string[];
    }> = [];
    const unsubscribe = projectEvents.subscribe((event) => observed.push({
      kind: event.kind,
      changedEntryKeys: [...event.changedEntryKeys],
    }));
    try {
      await useApp.getState().refreshEntriesAfterExternalChange([active.key]);
    } finally {
      unsubscribe();
    }

    expect(rpc.mock.calls.map(([method, params]) => [method, params])).toEqual([
      ["entry.list", undefined],
      ["stats.get", undefined],
      ["matches.query", { index: 1 }],
      ["glossary.query", { index: 1 }],
      ["issues.list", undefined],
    ]);
    expect(rpc.mock.calls.some(([method]) => method === "entry.set")).toBe(false);
    expect(observed).toEqual([
      {
        kind: "external-refresh",
        changedEntryKeys: [JSON.stringify(active.key)],
      },
      {
        kind: "entry",
        changedEntryKeys: [],
      },
    ]);
    expect({
      index: useApp.getState().index,
      key: useApp.getState().entries[1]?.key,
      translation: useApp.getState().document3.translation,
      note: useApp.getState().note,
      dirty: useApp.getState().document3.dirty,
      selection: useApp.getState().editorSelection,
      stats: useApp.getState().stats,
    }).toEqual({
      index: 1,
      key: active.key,
      translation: "fixed",
      note: "fixed note",
      dirty: false,
      selection: { anchor: 3, focus: 5 },
      stats,
    });
  });

  it("reloads team/filesystem state before rebinding the active Document3", async () => {
    const props = {
      root: "/watched",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      has_repositories: true,
    };
    const refreshed = {
      ...sampleEntry,
      source: "changed on disk",
      translation: "depuis disque",
      key: {
        ...sampleEntry.key,
        source_text: "changed on disk",
        path: "/watched",
      },
      translated: true,
      revision: 4,
    };
    const stats = {
      files: 1,
      segments: 1,
      translated: 1,
      unique_segments: 1,
      source_words: 3,
      target_words: 2,
    };
    rpc.mockImplementation(async (method: string) => {
      if (method === "project.external-refresh") return { props, entries: 1 };
      if (method === "entry.list") return [refreshed];
      if (method === "stats.get") return stats;
      if (
        method === "matches.query"
        || method === "glossary.query"
        || method === "issues.list"
      ) return [];
      throw new Error(`unexpected RPC: ${method}`);
    });
    useApp.setState({
      props,
      screen: "workspace",
      entries: [{ ...sampleEntry, translation: "stale server value" }],
      index: 0,
      note: "stale note",
      document3: createDocument3(sampleEntry.source, "unsaved stale renderer value"),
      prefs: defaultPreferences({
        insert_best_match: false,
        dictionary_auto_search: false,
      }),
      completerAuto: false,
    });

    await useApp.getState().refreshEntriesAfterExternalChange(undefined, true);

    expect(rpc.mock.calls.map(([method]) => method)).toEqual([
      "project.external-refresh",
      "entry.list",
      "stats.get",
      "matches.query",
      "glossary.query",
      "issues.list",
    ]);
    expect(rpc.mock.calls.some(([method]) => method === "entry.set")).toBe(false);
    expect({
      event: useApp.getState().projectEvent.kind,
      source: useApp.getState().document3.source,
      translation: useApp.getState().document3.translation,
      dirty: useApp.getState().document3.dirty,
      key: useApp.getState().entries[0]?.key,
    }).toEqual({
      event: "entry",
      source: "changed on disk",
      translation: "depuis disque",
      dirty: false,
      key: refreshed.key,
    });
  });

  it("rejects queued proactive events from an older same-root generation", async () => {
    const root = "/same-root";
    const refresh = vi.fn(async () => undefined);
    let notify: ((event: {
      root: string;
      paths: string[];
      generation: number;
      sources: Array<"native" | "sidecar">;
    }) => void) | undefined;
    window.omegat!.onProjectExternalChange = (listener) => {
      notify = listener;
      return () => {
        notify = undefined;
      };
    };
    projectEvents.publishProject("load", root);
    useApp.setState({
      props: {
        root,
        source_lang: "en",
        target_lang: "fr",
        sentence_seg: true,
        has_repositories: false,
      },
      refreshEntriesAfterExternalChange: refresh,
    });
    const generation = useApp.getState().projectEvent.projectGeneration;
    const disconnect = connectExternalProjectEvents();

    notify?.({
      root,
      paths: [`${root}/source/stale.txt`],
      generation: generation - 1,
      sources: ["sidecar"],
    });
    expect(refresh).not.toHaveBeenCalled();

    notify?.({
      root,
      paths: [`${root}/source/current.txt`],
      generation,
      sources: ["native", "sidecar"],
    });
    await vi.waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(refresh).toHaveBeenCalledWith(undefined, true);
    disconnect();
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
      note: "",
      document3: createDocument3("one", ""),
      filterUntranslated: true,
      editorFilter: { kind: "untranslated" },
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
      draft: useApp.getState().document3.translation,
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
      note: "",
      document3: createDocument3("one", ""),
    });
    useApp.getState().setDraft("不要丢失");

    await expect(useApp.getState().jump("next")).rejects.toThrow("optimistic revision conflict");
    expect({
      active: useApp.getState().index,
      draft: useApp.getState().document3.translation,
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
      key: sampleEntry.key,
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
      note: "local note",
      document3: createDocument3("same", "local edit"),
    });

    await expect(useApp.getState().commitCurrent()).rejects.toThrow(
      "optimistic lock failed for entry 0",
    );
    expect(useApp.getState().editConflict).toEqual({
      index: 0,
      key: sampleEntry.key,
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
      draft: useApp.getState().document3.translation,
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

  it("rebinds a duplicated-source editor conflict by complete EntryKey after reorder", async () => {
    const wantedKey = {
      ...sampleEntry.key,
      source_text: "same",
      prev: "before",
      next: "after",
      path: "/wanted",
    };
    const decoy = {
      ...sampleEntry,
      index: 0,
      key: { ...wantedKey, path: "/other" },
      source: "same",
      translation: "wrong duplicate",
      revision: 12,
      translated: true,
    };
    const remote = {
      ...sampleEntry,
      index: 1,
      key: wantedKey,
      source: "same",
      translation: "remote wanted",
      revision: 8,
      translated: true,
    };
    const committed = {
      ...remote,
      translation: "local wanted",
      note: "local note",
      revision: 9,
    };
    rpc.mockImplementation(async (method: string, params?: unknown) => {
      if (method === "entry.list") return [decoy, remote];
      if (method === "entry.set") {
        expect(params).toEqual({
          index: 1,
          key: wantedKey,
          translation: "local wanted",
          note: "local note",
          revision: 8,
          default_translation: true,
        });
        return { entry: committed, updated: [committed] };
      }
      throw new Error(`unexpected RPC: ${method}`);
    });
    useApp.setState({
      entries: [{ ...remote, index: 0, translation: "base", revision: 7 }],
      index: 0,
      note: "local note",
      document3: createDocument3("same", "local wanted"),
      editConflict: {
        index: 0,
        key: wantedKey,
        source: "same",
        previous: "base",
        ours: "local wanted",
        theirs: "remote wanted",
        note: "local note",
        default_translation: true,
        remote_revision: 8,
      },
    });

    await useApp.getState().resolveEditConflict("ours");

    expect({
      methods: rpc.mock.calls.map(([method]) => method),
      index: useApp.getState().index,
      keys: useApp.getState().entries.map((entry) => entry.key),
      document: useApp.getState().document3,
      conflict: useApp.getState().editConflict,
    }).toEqual({
      methods: ["entry.list", "entry.set", "issues.list"],
      index: 1,
      keys: [decoy.key, wantedKey],
      document: {
        ...createDocument3("same", "local wanted"),
        activeStart: "local wanted".length,
        activeEnd: "local wanted".length,
        dirty: false,
      },
      conflict: null,
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
      note: "local note",
      document3: createDocument3("same", "local edit"),
      editConflict: {
        index: 0,
        key: sampleEntry.key,
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
      draft: useApp.getState().document3.translation,
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
      document3: createDocument3(
        sampleEntry.source,
        "Hello <f0>world</f0>",
      ),
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
