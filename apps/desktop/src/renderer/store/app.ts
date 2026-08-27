import { create } from "zustand";
import { createDocument3, replaceEditText, type Document3State } from "../editor/Document3";
import { issuesForEntryOnLeave } from "../editor/EditorController";
import {
  findCyclicEntryIndex,
  findEntryBySourceAndKey,
  rebindEntryAfterReload,
  sameCompleteEntryKey,
} from "../editor/EditorNavigation";
import { IEditor } from "../editor/IEditor";
import { makeFilter, type EditorFilterState } from "../editor/IEditorFilter";
import {
  marksFromPrefs,
  nextMissingTag,
  prefsFromMarks,
  pushUndo,
  redoDraft,
  switchCase,
  undoDraft,
  type HistoryStacks,
  type ViewMarks,
} from "../lib/editor-doc";
import { DEFAULT_DOCK_LAYOUT, layoutFromPrefs, layoutToPrefs, serializeDockLayout, type DockLayout } from "../lib/layout";
import { DockProjectLifecycle, LatestDockRequest } from "../lib/dock-controllers";
import { applyColorVars, defaultPreferences } from "../lib/preferences";
import { projectEvents, type ProjectEvent } from "../lib/project-events";
import { defaultSearchForm, persistSearchForm, restoreSearchForm, type SearchForm } from "../lib/search-params";
import type {
  CompleterItemDto,
  DictHitDto,
  EditorConflict,
  EntryDto,
  EntryKeyDto,
  EntrySetResult,
  FilterInfoDto,
  GlossaryHitDto,
  IssueDto,
  MatchDto,
  MtSuggestionDto,
  Preferences,
  ProjectPropsDto,
  SearchHitDto,
  StatsDto,
  TeamConflict,
  WindowId,
} from "../lib/types";
import { applyDocumentLocale, detectLocale, t } from "../i18n";
import {
  LONG_OPERATION_METHODS,
  longOperationKindForMethod,
  type LongOperationKind,
  type RpcOperationEvent,
  type RpcOperationPhase,
} from "../../shared/rpc-operation";

function readLocal(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeLocal(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}

let nextRpcRequestId = 1;
let nextLongOperationId = 1;

async function rpc<T>(
  method: string,
  params?: unknown,
  signal?: AbortSignal,
  clientRequestId?: string,
): Promise<T> {
  if (!window.omegat) {
    throw new Error("sidecar bridge unavailable");
  }
  if (!signal) {
    try {
      return await (
        clientRequestId
          ? window.omegat.rpc(method, params, clientRequestId)
          : window.omegat.rpc(method, params)
      ) as T;
    } catch (error) {
      // Electron serializes ipcMain handler failures as ordinary Error
      // instances and drops Error.name. The sidecar's -32800 response has the
      // exact "request cancelled" contract; reconstruct AbortError only after
      // that response rejects the still-pending RPC.
      if (
        clientRequestId
        && error instanceof Error
        && error.message.toLowerCase().includes("request cancelled")
      ) {
        throw abortError(error.message);
      }
      throw error;
    }
  }
  stopIfCancelled(signal);
  const requestId = `renderer-${nextRpcRequestId++}`;
  const cancel = () => {
    void window.omegat.cancelRpc?.(requestId);
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    return await window.omegat.rpc(method, params, requestId) as T;
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}

function stopIfCancelled(signal: AbortSignal): void {
  if (!signal.aborted) return;
  const error = new Error("dock request cancelled");
  error.name = "AbortError";
  throw error;
}

function abortError(message: string): Error {
  const error = new Error(message);
  error.name = "AbortError";
  return error;
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError";
}

export type RendererLongOperation = {
  requestId: string;
  kind: LongOperationKind;
  method: string;
  phase: RpcOperationPhase;
  stage: string | null;
  error: string | null;
};

function applyRpcOperationEvent(
  current: RendererLongOperation | null,
  event: RpcOperationEvent,
): RendererLongOperation | null {
  const kind = longOperationKindForMethod(event.method);
  if (kind === null) return current;
  if (event.phase !== "started" && current?.requestId !== event.requestId) {
    return current;
  }
  if (current?.phase === "cancelling" && event.phase === "progress") {
    return current;
  }
  if (
    event.phase === "started"
    && current
    && current.requestId !== event.requestId
    && (current.phase === "started"
      || current.phase === "progress"
      || current.phase === "cancelling")
  ) {
    return current;
  }
  return {
    requestId: event.requestId,
    kind,
    method: event.method,
    phase: event.phase,
    stage: event.stage ?? current?.stage ?? null,
    error: event.error ?? (event.phase === "failed" ? "operation failed" : null),
  };
}

type SelectedDockData = {
  index: number;
  key: EntryDto["key"];
  matches: MatchDto[];
  glossary: GlossaryHitDto[];
  issues: IssueDto[];
  mt: MtSuggestionDto[];
  dict: DictHitDto[];
  completer: CompleterItemDto[];
  translation: string;
  note: string;
  source: string;
  file: string;
  previousIndex: number;
  navBack: number[];
  recordHistory: boolean;
  selection: { anchor: number; focus: number };
};

const selectedDockRequest = new LatestDockRequest<SelectedDockData>();
const mtDockRequest = new LatestDockRequest<MtSuggestionDto>();
const dictionaryDockRequest = new LatestDockRequest<DictHitDto[]>();
const completerDockRequest = new LatestDockRequest<CompleterItemDto[]>();
const searchDockRequest = new LatestDockRequest<SearchHitDto[]>();
const dockLifecycle = new DockProjectLifecycle([
  selectedDockRequest,
  mtDockRequest,
  dictionaryDockRequest,
  completerDockRequest,
  searchDockRequest,
], projectEvents);

function entryLifecycleKey(key: EntryDto["key"]): string {
  return JSON.stringify(key);
}

function clearedDockData() {
  return {
    matches: [] as MatchDto[],
    glossary: [] as GlossaryHitDto[],
    issues: [] as IssueDto[],
    mt: [] as MtSuggestionDto[],
    dict: [] as DictHitDto[],
    completer: [] as CompleterItemDto[],
    selectedMatch: 0,
  };
}

function normalizeEntrySetResult(result: EntrySetResult | EntryDto): EntrySetResult {
  if (
    "entry" in result
    && "updated" in result
    && Array.isArray(result.updated)
  ) {
    return result as EntrySetResult;
  }
  const entry = result as EntryDto;
  return { entry, updated: [entry] };
}

function isOptimisticLock(error: unknown): boolean {
  return /optimistic(?: lock| revision)/i.test(String(error));
}

type Screen = "welcome" | "workspace";

type ProjectRebindRequest = {
  kind: "reload" | "external-refresh" | "memory";
  changedKeys?: readonly EntryKeyDto[];
};

export type AppState = {
  screen: Screen;
  version: string;
  props: ProjectPropsDto | null;
  entries: EntryDto[];
  index: number;
  matches: MatchDto[];
  glossary: GlossaryHitDto[];
  stats: StatsDto | null;
  issues: IssueDto[];
  theme: "light" | "dark";
  error: string | null;
  document3: Document3State;
  editorSelection: { anchor: number; focus: number };
  editorFilter: EditorFilterState;
  projectEvent: ProjectEvent;
  note: string;
  firstRun: boolean;
  locale: string;
  mt: MtSuggestionDto[];
  dict: DictHitDto[];
  completer: CompleterItemDto[];
  filters: FilterInfoDto[];
  prefs: Preferences | null;
  teamMessage: string;
  teamConflicts: TeamConflict[];
  editConflict: EditorConflict | null;
  history: HistoryStacks;
  navBack: number[];
  navForward: number[];
  selectedMatch: number;
  selectedText: string;
  marks: ViewMarks;
  layout: DockLayout;
  windows: Partial<Record<WindowId, boolean>>;
  searchForm: SearchForm;
  searchHits: SearchHitDto[];
  log: string[];
  tipIndex: number;
  focusPanel: "editor" | "notes";
  filterUntranslated: boolean;
  completerAuto: boolean;
  historyCompletion: boolean;
  historyPrediction: boolean;
  mtAutoFetch: boolean;
  status: string;
  longOperation: RendererLongOperation | null;
  runLongOperation: <T>(
    kind: LongOperationKind,
    params?: Record<string, unknown>,
  ) => Promise<T>;
  cancelLongOperation: () => Promise<boolean>;
  applyPrefs: (p: Preferences) => void;
  setLocale: (locale: string) => void;
  logLine: (line: string) => void;
  openWindow: (id: WindowId, open?: boolean) => void;
  loadVersion: () => Promise<void>;
  open: (root: string) => Promise<void>;
  create: (root: string, sl: string, tl: string, seg: boolean) => Promise<void>;
  closeProject: () => Promise<void>;
  reloadProject: () => Promise<void>;
  rebindProjectEntries: (request: ProjectRebindRequest) => Promise<boolean>;
  select: (index: number, recordHistory?: boolean) => Promise<void>;
  setDraft: (v: string) => void;
  applyEditorDocument: (
    document: Document3State,
    selection?: { anchor: number; focus: number },
  ) => void;
  setEditorSelection: (
    selection:
      | { anchor: number; focus: number }
      | ((current: { anchor: number; focus: number }) => { anchor: number; focus: number }),
  ) => void;
  setNote: (v: string) => void;
  commitCurrent: (opts?: { default_translation?: boolean }) => Promise<EntryDto | null>;
  commit: (opts?: { default_translation?: boolean }) => Promise<void>;
  save: () => Promise<void>;
  compile: (file?: string) => Promise<void>;
  insertMatch: (n?: number, mode?: "overwrite" | "insert") => void;
  insertMt: (mode?: "overwrite" | "insert") => void;
  insertSource: (mode?: "overwrite" | "insert") => void;
  insertTag: () => void;
  insertChar: (ch: string) => void;
  undo: () => void;
  redo: () => void;
  applyCase: (mode: "upper" | "lower" | "title" | "sentence" | "cycle") => void;
  registerEmpty: () => Promise<void>;
  registerIdentical: () => Promise<void>;
  registerUntranslated: () => Promise<void>;
  jump: (kind: "next" | "prev" | "untranslated" | "translated" | "unique" | "note" | "auto" | "enforce" | "number", n?: number, dir?: 1 | -1) => Promise<void>;
  selectSource: () => void;
  exportSelection: () => Promise<void>;
  importFiles: () => Promise<void>;
  importPaths: (paths: string[]) => Promise<void>;
  clearRecent: () => void;
  exitApp: () => Promise<void>;
  restartApp: () => Promise<void>;
  runScriptSlot: (slot: number) => Promise<void>;
  gotoMatchSource: () => Promise<void>;
  insertAllTags: () => void;
  historyBack: () => Promise<void>;
  historyForward: () => Promise<void>;
  toggleMark: (key: keyof ViewMarks) => Promise<void>;
  setModification: (v: ViewMarks["modification"]) => Promise<void>;
  setLayout: (partial: Partial<DockLayout>) => void;
  restoreLayout: () => void;
  persistMarksAndLayout: () => Promise<void>;
  queryMt: (engine?: string) => Promise<void>;
  queryDict: (word: string) => Promise<void>;
  queryCompleter: (prefix: string) => Promise<void>;
  loadFilters: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  savePrefs: (p: Preferences) => Promise<void>;
  patchPrefs: (patch: Partial<Preferences>) => Promise<void>;
  runSearch: (preview?: boolean) => Promise<SearchHitDto[]>;
  replaceAll: () => Promise<number>;
  teamSync: () => Promise<void>;
  teamCommit: (which: "source" | "target") => Promise<void>;
  resolveConflict: (
    side: "ours" | "theirs" | "manual",
    source?: string,
    translation?: string,
    entryKey?: EntryKeyDto,
  ) => Promise<void>;
  resolveEditConflict: (side: "ours" | "theirs" | "manual", translation?: string) => Promise<void>;
  learnWord: (word: string) => Promise<void>;
  ignoreWord: (word: string) => Promise<void>;
  addGlossary: (source: string, target: string, comment?: string) => Promise<void>;
  importWiki: (source: string) => Promise<void>;
  refreshEntriesAfterExternalChange: (
    changedKeys?: readonly EntryKeyDto[],
    reloadFromDisk?: boolean,
  ) => Promise<boolean>;
  toggleTheme: () => void;
  setSearchForm: (patch: Partial<SearchForm>) => void;
};

const emptyWindows: Partial<Record<WindowId, boolean>> = {};

const initialState = {
  screen: "welcome" as Screen,
  version: "",
  props: null as ProjectPropsDto | null,
  entries: [] as EntryDto[],
  index: 0,
  matches: [] as MatchDto[],
  glossary: [] as GlossaryHitDto[],
  stats: null as StatsDto | null,
  issues: [] as IssueDto[],
  theme: "light" as const,
  error: null as string | null,
  document3: createDocument3("", ""),
  editorSelection: { anchor: 0, focus: 0 },
  editorFilter: { kind: "none" } as EditorFilterState,
  projectEvent: projectEvents.current(),
  note: "",
  mt: [] as MtSuggestionDto[],
  dict: [] as DictHitDto[],
  completer: [] as CompleterItemDto[],
  filters: [] as FilterInfoDto[],
  prefs: null as Preferences | null,
  teamMessage: "",
  teamConflicts: [] as TeamConflict[],
  editConflict: null as EditorConflict | null,
  history: { undo: [] as string[], redo: [] as string[] },
  navBack: [] as number[],
  navForward: [] as number[],
  selectedMatch: 0,
  selectedText: "",
  marks: marksFromPrefs(undefined),
  layout: { ...DEFAULT_DOCK_LAYOUT },
  windows: { ...emptyWindows },
  searchForm: defaultSearchForm(),
  searchHits: [] as SearchHitDto[],
  log: [] as string[],
  tipIndex: 0,
  focusPanel: "editor" as const,
  filterUntranslated: false,
  completerAuto: true,
  historyCompletion: true,
  historyPrediction: true,
  mtAutoFetch: false,
  status: "",
  longOperation: null as RendererLongOperation | null,
};

function bindTeamConflictEntries(
  conflicts: readonly TeamConflict[],
  entries: readonly EntryDto[],
  activeIndex: number,
): TeamConflict[] {
  const active = entries[activeIndex];
  return conflicts.map((conflict) => {
    if (
      conflict.entry_key
      && entries.some((entry) =>
        sameCompleteEntryKey(entry.key, conflict.entry_key)
      )
    ) {
      return conflict;
    }
    if (!conflict.source) return conflict;
    const matches = entries.filter((entry) => entry.source === conflict.source);
    const entry = active?.source === conflict.source
      ? active
      : matches.length === 1
        ? matches[0]
        : undefined;
    return entry
      ? { ...conflict, entry_key: { ...entry.key } }
      : { ...conflict, entry_key: undefined };
  });
}

function reboundProjectState(
  before: AppState,
  entries: EntryDto[],
  stats: StatsDto,
  props: ProjectPropsDto | null,
): { patch: Partial<AppState>; index: number } {
  const previous = before.entries[before.index];
  const binding = rebindEntryAfterReload(
    entries,
    before.index,
    (entry) => sameCompleteEntryKey(entry.key, previous?.key),
  );
  if (binding.index < 0) {
    return {
      index: -1,
      patch: {
        ...clearedDockData(),
        entries,
        stats,
        props,
        index: 0,
        note: "",
        document3: createDocument3("", ""),
        editorSelection: { anchor: 0, focus: 0 },
        history: { undo: [], redo: [] },
        navBack: [],
        navForward: [],
        editConflict: null,
        teamConflicts: bindTeamConflictEntries(
          before.teamConflicts,
          entries,
          0,
        ),
        status: "",
      },
    };
  }

  const entry = entries[binding.index]!;
  const limit = entry.translation.length;
  return {
    index: binding.index,
    patch: {
      ...clearedDockData(),
      entries,
      stats,
      props,
      index: binding.index,
      note: entry.note,
      document3: createDocument3(entry.source, entry.translation),
      editorSelection: binding.exact
        ? {
            anchor: Math.max(0, Math.min(before.editorSelection.anchor, limit)),
            focus: Math.max(0, Math.min(before.editorSelection.focus, limit)),
          }
        : { anchor: limit, focus: limit },
      history: { undo: [], redo: [] },
      navBack: [],
      navForward: [],
      editConflict: null,
      teamConflicts: bindTeamConflictEntries(
        before.teamConflicts,
        entries,
        binding.index,
      ),
      status: "",
    },
  };
}

export const useApp = create<AppState>((set, get) => ({
  ...initialState,
  firstRun: !readLocal("omegat.first"),
  locale: (() => {
    const saved = readLocal("omegat.locale");
    const nav = typeof navigator !== "undefined" ? navigator.language : "en";
    const loc = detectLocale(saved || nav);
    applyDocumentLocale(loc);
    return loc;
  })(),
  runLongOperation: async <T,>(
    kind: LongOperationKind,
    params: Record<string, unknown> = {},
  ): Promise<T> => {
    const active = get().longOperation;
    if (
      active
      && (active.phase === "started"
        || active.phase === "progress"
        || active.phase === "cancelling")
    ) {
      void window.omegat.cancelRpc?.(active.requestId);
    }
    const method = LONG_OPERATION_METHODS[kind];
    const requestId = `operation-${kind}-${nextLongOperationId++}`;
    set({
      longOperation: {
        requestId,
        kind,
        method,
        phase: "started",
        stage: null,
        error: null,
      },
    });
    try {
      const result = await rpc<T>(
        method,
        { ...params, progress_token: requestId },
        undefined,
        requestId,
      );
      const current = get().longOperation;
      if (
        current?.requestId === requestId
        && current.phase === "cancelled"
      ) {
        throw abortError(`${kind} cancelled`);
      }
      if (current?.requestId === requestId) {
        set({ longOperation: { ...current, phase: "succeeded", error: null } });
      }
      return result;
    } catch (error) {
      const current = get().longOperation;
      const cancelled = isAbortError(error)
        || (
          current?.requestId === requestId
          && current.phase === "cancelled"
        );
      if (current?.requestId === requestId) {
        set({
          longOperation: {
            ...current,
            phase: cancelled ? "cancelled" : "failed",
            error: cancelled ? null : String(error),
          },
        });
      }
      if (cancelled && !isAbortError(error)) {
        throw abortError(`${kind} cancelled`);
      }
      throw error;
    }
  },
  cancelLongOperation: async () => {
    const active = get().longOperation;
    if (
      !active
      || (active.phase !== "started" && active.phase !== "progress")
      || !window.omegat.cancelRpc
    ) {
      return false;
    }
    set({ longOperation: { ...active, phase: "cancelling" } });
    const accepted = await window.omegat.cancelRpc(active.requestId);
    const current = get().longOperation;
    if (
      !accepted
      && current?.requestId === active.requestId
      && current.phase === "cancelling"
    ) {
      set({
        longOperation: {
          ...current,
          phase: "failed",
          error: "operation is no longer active",
        },
      });
    }
    return accepted;
  },
  applyPrefs: (p) => {
    const prefs = defaultPreferences(p);
    applyDocumentLocale(prefs.locale || get().locale);
    void window.omegat?.setMenuLocale?.(prefs.locale || get().locale);
    const theme = (prefs.theme === "dark" ? "dark" : "light") as "light" | "dark";
    if (typeof document !== "undefined") {
      document.documentElement.dataset.theme = theme;
      document.documentElement.style.setProperty("--font", `"${prefs.font_ui || "IBM Plex Sans"}", sans-serif`);
      document.documentElement.style.setProperty("--font-editor", `"${prefs.font_editor || "IBM Plex Sans"}", sans-serif`);
    }
    applyColorVars(prefs.colors);
    set({
      prefs,
      theme,
      locale: prefs.locale || get().locale,
      firstRun: !prefs.first_time_wizard_done,
      marks: marksFromPrefs(prefs.marks),
      layout: layoutFromPrefs(prefs.docking_layout, readLocal("omegat.layout")),
      searchForm: { ...restoreSearchForm(prefs.search_window), query: get().searchForm.query, replace: get().searchForm.replace },
      filterUntranslated: prefs.filter_untranslated,
      editorFilter: prefs.filter_untranslated
        ? { kind: "untranslated" }
        : { kind: "none" },
      completerAuto: prefs.completer_auto,
      historyCompletion: prefs.history_completion,
      historyPrediction: prefs.history_prediction,
      mtAutoFetch: prefs.mt_auto_fetch,
    });
  },
  setLocale: (locale) => {
    applyDocumentLocale(locale);
    writeLocal("omegat.locale", locale);
    set({ locale });
    void window.omegat?.setMenuLocale?.(locale);
    const prefs = get().prefs;
    if (prefs) void get().savePrefs({ ...prefs, locale });
  },
  logLine: (line) => set({ log: [...get().log.slice(-400), `${new Date().toISOString()} ${line}`] }),
  openWindow: (id, open = true) => set({ windows: { ...get().windows, [id]: open } }),
  loadVersion: async () => {
    try {
      const v = await rpc<{ version: string }>("sys.version");
      set({ version: v.version });
      get().logLine(`sidecar ${v.version}`);
    } catch (e) {
      set({ error: String(e) });
    }
  },
  open: async (root) => {
    const lifecycle = dockLifecycle.beginProject(root, "load");
    set({ ...clearedDockData(), error: null, status: "" });
    const props = await rpc<ProjectPropsDto>("project.open", { root });
    if (!dockLifecycle.isCurrent(lifecycle)) return;
    const listed = await rpc<EntryDto[]>("entry.list");
    if (!dockLifecycle.isCurrent(lifecycle)) return;
    const entries = Array.isArray(listed) ? listed : [];
    const stats = await rpc<StatsDto>("stats.get");
    if (!dockLifecycle.isCurrent(lifecycle)) return;
    let teamConflicts: TeamConflict[] = [];
    if (props.has_repositories) {
      try {
        const queued = await rpc<{ conflicts: TeamConflict[] | string[] }>(
          "team.conflicts",
        );
        if (!dockLifecycle.isCurrent(lifecycle)) return;
        teamConflicts = Array.isArray(queued.conflicts)
          ? queued.conflicts.map((item) =>
              typeof item === "string" ? { message: item } : item
            )
          : [];
      } catch {
        if (!dockLifecycle.isCurrent(lifecycle)) return;
      }
    }
    const firstEntry = entries[0];
    const firstTranslation = firstEntry?.translation ?? "";
    set({
      props,
      entries,
      screen: "workspace",
      index: 0,
      stats,
      error: null,
      note: firstEntry?.note ?? "",
      document3: createDocument3(firstEntry?.source ?? "", firstTranslation),
      editorSelection: {
        anchor: firstTranslation.length,
        focus: firstTranslation.length,
      },
      teamConflicts: bindTeamConflictEntries(teamConflicts, entries, 0),
    });
    await get().loadPrefs();
    if (!dockLifecycle.isCurrent(lifecycle)) return;
    await get().select(0, false);
    if (get().props?.root !== root) return;
    const rec = JSON.parse(readLocal("omegat.recent") || "[]") as string[];
    writeLocal("omegat.recent", JSON.stringify([root, ...rec.filter((r) => r !== root)].slice(0, 8)));
    writeLocal("omegat.first", "1");
    if (get().prefs) {
      await get().patchPrefs({ first_time_wizard_done: true });
    }
    set({ firstRun: false });
    if (get().prefs?.project_files_show_on_load) get().openWindow("files");
    get().logLine(`opened ${root}`);
  },
  create: async (root, sl, tl, seg) => {
    const lifecycle = dockLifecycle.beginProject(root, "create");
    set({ ...clearedDockData(), error: null, status: "" });
    await rpc("project.create", { root, source_lang: sl, target_lang: tl, sentence_seg: seg });
    if (!dockLifecycle.isCurrent(lifecycle)) return;
    await get().open(root);
  },
  closeProject: async () => {
    dockLifecycle.beginProject(null, "close");
    const { locale, theme, version } = get();
    set({
      ...initialState,
      locale,
      theme,
      version,
      firstRun: false,
      screen: "welcome",
    });
    try {
      await rpc("project.close");
    } catch {
      /* ignore */
    }
  },
  reloadProject: async () => {
    await get().rebindProjectEntries({ kind: "reload" });
  },
  rebindProjectEntries: async ({ kind, changedKeys }) => {
    const root = get().props?.root;
    if (!root) return false;
    const initial = get();
    const previous = initial.entries[initial.index];
    const lifecycle = kind === "reload"
      ? dockLifecycle.beginProject(root, "reload")
      : dockLifecycle.externalRefresh(
          root,
          previous ? entryLifecycleKey(previous.key) : null,
          changedKeys?.map(entryLifecycleKey) ?? [],
        );

    if (kind === "reload") {
      if (
        previous
        && (
          initial.document3.translation !== previous.translation
          || initial.note !== previous.note
        )
      ) {
        await get().commitCurrent();
        if (!dockLifecycle.isCurrent(lifecycle)) return false;
      }
      await rpc("project.save");
      if (!dockLifecycle.isCurrent(lifecycle)) return false;
    }

    const before = get();
    let refreshedProps: ProjectPropsDto | null = before.props;
    try {
      if (kind === "reload") {
        const result = await get().runLongOperation<{ props?: ProjectPropsDto }>("reload");
        refreshedProps = result.props ?? before.props;
      } else if (kind === "external-refresh") {
        const result = await get().runLongOperation<{ props?: ProjectPropsDto }>(
          "externalRefresh",
        );
        refreshedProps = result.props ?? before.props;
      }
    } catch (error) {
      if (!isAbortError(error)) throw error;
      if (get().props?.root === root) {
        set({
          status: kind === "reload"
            ? "reload cancelled"
            : "external refresh cancelled",
        });
      }
      return false;
    }

    if (!dockLifecycle.isCurrent(lifecycle)) return false;
    const listed = await rpc<EntryDto[]>("entry.list");
    if (!dockLifecycle.isCurrent(lifecycle)) return false;
    const entries = Array.isArray(listed) ? listed : [];
    const stats = await rpc<StatsDto>("stats.get");
    if (!dockLifecycle.isCurrent(lifecycle)) return false;

    // Publish entries, active Document3, note, caret and navigation reset as
    // one complete-key transaction. No candidate list is visible before this
    // point, including when the sidecar acknowledges cancellation.
    const rebound = reboundProjectState(
      before,
      entries,
      stats,
      refreshedProps,
    );
    set(rebound.patch);
    if (rebound.index >= 0) await get().select(rebound.index, false);
    if (kind === "reload") get().logLine("reloaded project");
    return true;
  },
  select: async (index, recordHistory = true) => {
    const before = get();
    const previousEntry = before.entries[before.index];
    if (
      previousEntry
      && (
        before.document3.translation !== previousEntry.translation
        || before.note !== previousEntry.note
      )
    ) {
      // A failed optimistic write rejects the navigation and leaves the live
      // Document3 untouched instead of silently replacing the user's draft.
      await get().commitCurrent();
    }
    const {
      entries,
      index: prev,
      navBack,
      prefs,
      editorSelection,
    } = get();
    const insert_best = prefs?.insert_best_match ?? true;
    const e = entries[index];
    if (!e) return;
    const lifecycle = dockLifecycle.activateEntry(
      get().props?.root ?? null,
      entryLifecycleKey(e.key),
    );
    set({ ...clearedDockData(), selectedText: "" });
    await selectedDockRequest.run(async (signal) => {
      const matches = await rpc<MatchDto[]>("matches.query", { index }, signal);
      stopIfCancelled(signal);
      const glossary = await rpc<GlossaryHitDto[]>("glossary.query", { index }, signal);
      stopIfCancelled(signal);
      const issues = await rpc<IssueDto[]>("issues.list", undefined, signal);
      stopIfCancelled(signal);
      let mt: MtSuggestionDto[] = [];
      if (get().mtAutoFetch) {
        try {
          mt = [await rpc<MtSuggestionDto>(
            "mt.query",
            { index, engine: "mymemory" },
            signal,
          )];
        } catch {
          mt = [];
        }
        stopIfCancelled(signal);
      }
      const dict = get().prefs?.dictionary_auto_search
        ? await rpc<DictHitDto[]>("dict.query", {
            word: e.source.split(/\s+/)[0] || "",
            fuzzy: get().prefs?.dictionary_fuzzy_matching,
          }, signal)
        : [];
      stopIfCancelled(signal);
      let translation = e.translation;
      if (!translation && insert_best && matches[0]) {
        translation = matches[0].translation;
      }
      const sameEntry = sameCompleteEntryKey(entries[prev]?.key, e.key);
      const selection = sameEntry
        ? {
            anchor: Math.max(0, Math.min(editorSelection.anchor, translation.length)),
            focus: Math.max(0, Math.min(editorSelection.focus, translation.length)),
          }
        : { anchor: translation.length, focus: translation.length };
      const completer = get().completerAuto
        ? await rpc<CompleterItemDto[]>("completer.query", {
            index,
            prefix: "",
            text: translation,
          }, signal)
        : [];
      stopIfCancelled(signal);
      return {
        index,
        key: { ...e.key },
        matches,
        glossary,
        issues,
        mt,
        dict,
        completer,
        translation,
        note: e.note,
        source: e.source,
        file: e.file,
        previousIndex: prev,
        navBack,
        recordHistory,
        selection,
      };
    }, (loaded) => {
      if (!dockLifecycle.isCurrent(lifecycle)) return;
      const current = get().entries[loaded.index];
      if (!current || !sameCompleteEntryKey(current.key, loaded.key)) return;
      set({
        index: loaded.index,
        matches: loaded.matches,
        glossary: loaded.glossary,
        issues: loaded.issues,
        mt: loaded.mt,
        dict: loaded.dict,
        completer: loaded.completer,
        document3: createDocument3(loaded.source, loaded.translation),
        note: loaded.note,
        history: { undo: [], redo: [] },
        selectedMatch: 0,
        editorSelection: loaded.selection,
        status: `${loaded.file} #${loaded.index + 1}`,
        ...(loaded.recordHistory && loaded.previousIndex !== loaded.index
          ? { navBack: [...loaded.navBack, loaded.previousIndex], navForward: [] }
          : {}),
      });
    });
  },
  queryMt: async (engine = "mymemory") => {
    try {
      const state = get();
      const index = state.index;
      const entry = state.entries[index];
      if (!entry) return;
      const lifecycle = dockLifecycle.captureEntry(
        state.props?.root ?? null,
        entryLifecycleKey(entry.key),
      );
      await mtDockRequest.run(
        (signal) => rpc<MtSuggestionDto>("mt.query", { index, engine }, signal),
        (one) => {
          if (!dockLifecycle.isCurrent(lifecycle)) return;
          set({ mt: [one, ...get().mt.filter((m) => m.engine !== engine)] });
        },
      );
    } catch (e) {
      set({ error: String(e) });
    }
  },
  queryDict: async (word) => {
    const state = get();
    const index = state.index;
    const entry = state.entries[index];
    if (!entry) return;
    const lifecycle = dockLifecycle.captureEntry(
      state.props?.root ?? null,
      entryLifecycleKey(entry.key),
    );
    await dictionaryDockRequest.run((signal) => rpc<DictHitDto[]>("dict.query", {
        word,
        fuzzy: state.prefs?.dictionary_fuzzy_matching,
      }, signal), (dict) => {
        if (dockLifecycle.isCurrent(lifecycle)) set({ dict });
      });
  },
  queryCompleter: async (prefix) => {
    if (!get().completerAuto && !prefix) {
      set({ completer: [] });
      return;
    }
    const state = get();
    const index = state.index;
    const entry = state.entries[index];
    if (!entry) return;
    const lifecycle = dockLifecycle.captureEntry(
      state.props?.root ?? null,
      entryLifecycleKey(entry.key),
    );
    await completerDockRequest.run((signal) => rpc<CompleterItemDto[]>("completer.query", {
        index,
        prefix,
        text: state.document3.translation,
      }, signal), (completer) => {
        if (dockLifecycle.isCurrent(lifecycle)) set({ completer });
      });
  },
  loadFilters: async () => set({ filters: await rpc<FilterInfoDto[]>("filters.list") }),
  loadPrefs: async () => {
    const prefs = await rpc<Preferences>("prefs.get");
    get().applyPrefs(prefs);
  },
  savePrefs: async (p) => {
    const prefs = await rpc<Preferences>("prefs.set", p);
    get().applyPrefs(prefs && typeof prefs === "object" && "marks" in prefs ? prefs : p);
    get().logLine("saved preferences");
  },
  patchPrefs: async (patch) => {
    const cur = get().prefs;
    if (!cur) return;
    await get().savePrefs(defaultPreferences({ ...cur, ...patch }));
  },
  setSearchForm: (patch) => set({ searchForm: { ...get().searchForm, ...patch } }),
  runSearch: async (preview = false) => {
    const form = get().searchForm;
    let published: SearchHitDto[] = [];
    await searchDockRequest.run(
      (signal) => rpc<SearchHitDto[]>("search.run", {
        ...persistSearchForm(form),
        query: form.query,
        regex: form.searchType === "regex",
        search_type: form.searchType,
        source: form.source,
        translation: form.translation,
        notes: form.notes,
        comments: form.comments,
        case_sensitive: form.caseSensitive,
        whole_word: form.wholeWord,
        untranslated: form.untranslated,
        author: form.author || undefined,
        date_from: form.dateFrom || undefined,
        date_to: form.dateTo || undefined,
        replace: preview ? form.replace : undefined,
        preview,
      }, signal),
      (hits) => {
        published = hits;
        set({ searchHits: hits });
        if (get().prefs) {
          void get().patchPrefs({ search_window: persistSearchForm(form) });
        }
      },
    );
    return published;
  },
  replaceAll: async () => {
    const form = get().searchForm;
    const r = await rpc<{ replaced: number }>("search.replace", {
      query: form.query,
      replace: form.replace,
      regex: form.searchType === "regex",
      search_type: form.searchType,
      source: false,
      translation: form.translation,
      notes: form.notes,
      comments: false,
      case_sensitive: form.caseSensitive,
      whole_word: form.wholeWord,
      untranslated: form.untranslated,
      author: form.author || undefined,
      date_from: form.dateFrom || undefined,
      date_to: form.dateTo || undefined,
    });
    await get().refreshEntriesAfterExternalChange();
    get().logLine(`replaced ${r.replaced}`);
    return r.replaced;
  },
  teamSync: async () => {
    try {
      const r = await get().runLongOperation<{ action: string; message: string }>(
        "teamSync",
      );
      set({ teamMessage: `${r.action}: ${r.message}` });
      await get().refreshEntriesAfterExternalChange(undefined, true);
    } catch (e) {
      if (isAbortError(e)) {
        set({ teamMessage: "sync cancelled" });
        return;
      }
      const msg = String(e);
      set({ teamMessage: msg, error: msg });
      try {
        const c = await rpc<{ conflicts: TeamConflict[] | string[] }>("team.conflicts");
        const list = Array.isArray(c.conflicts)
          ? c.conflicts.map((x) => (typeof x === "string" ? { message: x } : x))
          : [];
        const current = get();
        set({
          teamConflicts: bindTeamConflictEntries(
            list,
            current.entries,
            current.index,
          ),
        });
      } catch {
        /* ignore */
      }
    }
  },
  teamCommit: async (which) => {
    try {
      const r = await get().runLongOperation<{ action: string; message: string }>(
        "teamCommit",
        { which },
      );
      set({ teamMessage: `${r.action}: ${r.message}` });
      get().logLine(`commit ${which}`);
    } catch (error) {
      if (!isAbortError(error)) throw error;
      set({ teamMessage: `commit ${which} cancelled` });
    }
  },
  resolveConflict: async (side, source, translation, entryKey) => {
    const before = get();
    const conflict = before.teamConflicts.find((item) =>
      entryKey
        ? sameCompleteEntryKey(item.entry_key, entryKey)
        : source
          ? item.source === source
          : true
    );
    const src = source ?? conflict?.source ?? "";
    const activeKey = before.entries[before.index]?.key;
    const rebindKey = entryKey ?? conflict?.entry_key ?? activeKey;
    let r: { conflicts: TeamConflict[] };
    try {
      r = await get().runLongOperation<{ conflicts: TeamConflict[] }>(
        "teamResolve",
        {
          source: src,
          side,
          translation,
          rebind_key: rebindKey,
        },
      );
    } catch (error) {
      if (!isAbortError(error)) throw error;
      set({ teamMessage: `resolve cancelled${src ? ` (${src})` : ""}` });
      return;
    }
    await get().refreshEntriesAfterExternalChange(
      rebindKey ? [rebindKey] : undefined,
      true,
    );
    const current = get();
    set({
      teamConflicts: bindTeamConflictEntries(
        r.conflicts ?? [],
        current.entries,
        current.index,
      ),
      teamMessage: `keep ${side}${src ? ` (${src})` : ""}`,
    });
    await get().patchPrefs({ team_conflict_resolution: side });
  },
  resolveEditConflict: async (side, translation) => {
    const conflict = get().editConflict;
    if (!conflict) return;
    const latest = await rpc<EntryDto[]>("entry.list");
    const remoteIndex = findEntryBySourceAndKey(
      latest,
      conflict.source,
      conflict.key,
    );
    const remote = latest[remoteIndex];
    if (!remote) {
      throw new Error("editor conflict entry is no longer available");
    }
    if (side === "theirs") {
      set({
        entries: latest,
        index: remoteIndex,
        note: remote.note,
        document3: createDocument3(remote.source, remote.translation),
        history: { undo: [], redo: [] },
        editConflict: null,
        error: null,
      });
      return;
    }
    const chosen = side === "manual" ? (translation ?? conflict.ours) : conflict.ours;
    set({
      entries: latest,
      index: remoteIndex,
      note: conflict.note,
      document3: replaceEditText(createDocument3(remote.source, remote.translation), chosen),
      editConflict: null,
      error: null,
    });
    await get().commitCurrent({
      default_translation: conflict.default_translation,
    });
  },
  learnWord: async (word) => {
    await rpc("spell.learn", { word });
    IEditor.remarkOneMarker("org.omegat.core.spellchecker.SpellCheckerMarker");
    await get().select(get().index, false);
  },
  ignoreWord: async (word) => {
    await rpc("spell.ignore", { word });
    IEditor.remarkOneMarker("org.omegat.core.spellchecker.SpellCheckerMarker");
    await get().select(get().index, false);
  },
  addGlossary: async (source, target, comment = "") => {
    await rpc("glossary.add", { source, target, comment });
    await get().select(get().index, false);
  },
  importWiki: async (source) => {
    await rpc("wiki.import", { source });
    await get().reloadProject();
  },
  refreshEntriesAfterExternalChange: async (changedKeys, reloadFromDisk = false) => {
    return get().rebindProjectEntries({
      kind: reloadFromDisk ? "external-refresh" : "memory",
      changedKeys,
    });
  },
  setDraft: (v) => {
    const prev = get().document3.translation;
    const src = get().entries[get().index]?.source ?? get().document3.source;
    const current = get().document3;
    const document3 = current.source === src
      ? replaceEditText(current, v)
      : replaceEditText(createDocument3(src, v), v);
    set({
      document3,
      history: pushUndo(get().history, prev, v),
    });
  },
  applyEditorDocument: (document3, selection) => {
    const current = get();
    const limit = document3.translation.length;
    const editorSelection = selection
      ? {
          anchor: Math.max(0, Math.min(selection.anchor, limit)),
          focus: Math.max(0, Math.min(selection.focus, limit)),
        }
      : current.editorSelection;
    set({
      document3,
      editorSelection,
      selectedText: "",
      history: pushUndo(
        current.history,
        current.document3.translation,
        document3.translation,
      ),
    });
  },
  setEditorSelection: (selection) => set({
    editorSelection: typeof selection === "function"
      ? selection(get().editorSelection)
      : selection,
    selectedText: "",
  }),
  setNote: (v) => set({ note: v }),
  undo: () => {
    const { draft, stacks } = undoDraft(
      get().history,
      get().document3.translation,
    );
    const src = get().entries[get().index]?.source ?? get().document3.source;
    set({ document3: createDocument3(src, draft), history: stacks });
  },
  redo: () => {
    const { draft, stacks } = redoDraft(
      get().history,
      get().document3.translation,
    );
    const src = get().entries[get().index]?.source ?? get().document3.source;
    set({ document3: createDocument3(src, draft), history: stacks });
  },
  applyCase: (mode) => get().setDraft(
    switchCase(get().document3.translation, mode),
  ),
  insertMatch: (n = 1, mode = "overwrite") => {
    const m = get().matches[(n ?? 1) - 1] ?? get().matches[get().selectedMatch];
    if (!m) return;
    if (mode === "insert") get().setDraft(
      get().document3.translation + m.translation,
    );
    else get().setDraft(m.translation);
    set({ selectedMatch: Math.max(0, (n ?? 1) - 1) });
  },
  insertMt: (mode = "overwrite") => {
    const m = get().mt[0];
    if (!m) return;
    if (mode === "insert") get().setDraft(get().document3.translation + m.text);
    else get().setDraft(m.text);
  },
  insertSource: (mode = "overwrite") => {
    const src = get().entries[get().index]?.source ?? "";
    if (mode === "insert") get().setDraft(get().document3.translation + src);
    else get().setDraft(src);
  },
  insertTag: () => {
    const e = get().entries[get().index];
    const translation = get().document3.translation;
    const tag = e ? nextMissingTag(e.source, translation) : null;
    if (tag) get().setDraft(translation + tag);
  },
  insertAllTags: () => {
    const e = get().entries[get().index];
    if (!e) return;
    let draft = get().document3.translation;
    let tag = nextMissingTag(e.source, draft);
    while (tag) {
      draft += tag;
      tag = nextMissingTag(e.source, draft);
    }
    get().setDraft(draft);
  },
  insertChar: (ch) => get().setDraft(get().document3.translation + ch),
  selectSource: () => {
    const src = get().entries[get().index]?.source ?? "";
    set({ selectedText: src, focusPanel: "editor" });
  },
  exportSelection: async () => {
    const text = get().selectedText || get().document3.translation;
    if (window.omegat?.saveText) {
      await window.omegat.saveText("selection.txt", text);
    }
    get().logLine(`exported selection (${text.length} chars)`);
  },
  importFiles: async () => {
    const files = (await window.omegat?.pickFiles?.()) ?? [];
    await get().importPaths(files);
  },
  importPaths: async (files) => {
    const paths = files.filter((path) => path.trim().length > 0);
    if (!paths.length || !get().props) return;
    await rpc("project.import", { files: paths });
    await get().reloadProject();
    get().logLine(`imported ${paths.length} file(s)`);
  },
  clearRecent: () => {
    writeLocal("omegat.recent", "[]");
    get().logLine("cleared recent projects");
  },
  exitApp: async () => {
    if (get().prefs?.always_confirm_quit) {
      const ok = typeof window !== "undefined" ? window.confirm("Quit OmegaT?") : true;
      if (!ok) return;
    }
    await window.omegat?.quit?.();
  },
  restartApp: async () => {
    await window.omegat?.relaunch?.();
  },
  runScriptSlot: async (slot) => {
    const src = get().prefs?.script_slots[slot - 1];
    if (src) {
      await rpc("script.run", { source: src, index: get().index });
    } else {
      await rpc("script.slot", { slot, index: get().index });
    }
    get().logLine(`ran script slot ${slot}`);
    try {
      await get().refreshEntriesAfterExternalChange();
    } catch {
      /* ignore */
    }
  },
  gotoMatchSource: async () => {
    const m = get().matches[get().selectedMatch];
    if (!m) return;
    const i = get().entries.findIndex((e) => e.source === m.source);
    if (i >= 0) await get().select(i);
  },
  registerEmpty: async () => {
    get().setDraft("");
    await get().commit();
  },
  registerIdentical: async () => {
    get().insertSource("overwrite");
    await get().commit();
  },
  registerUntranslated: async () => {
    get().setDraft("");
    await get().commit();
  },
  commitCurrent: async (opts) => {
    const {
      index,
      entries,
      document3,
      note,
    } = get();
    const e = entries[index];
    if (!e) return null;
    const translation = document3.translation;
    const defaultTranslation = opts?.default_translation ?? e.default_translation;
    try {
      const response = await rpc<EntrySetResult | EntryDto>("entry.set", {
        index,
        key: e.key,
        translation,
        note,
        revision: e.revision,
        default_translation: defaultTranslation,
      });
      const result = normalizeEntrySetResult(response);
      const updates = new Map(result.updated.map((entry) => [entry.index, entry]));
      updates.set(result.entry.index, result.entry);
      const next = entries.map((entry) => updates.get(entry.index) ?? entry);
      let leaveIssues: IssueDto[] = [];
      if (get().prefs?.tag_validation !== "none") {
        try {
          const allIssues = await rpc<IssueDto[]>("issues.list");
          leaveIssues = issuesForEntryOnLeave(
            result.entry,
            Array.isArray(allIssues) ? allIssues : [],
          );
        } catch {
          // Java runs leave checks asynchronously; a checker failure must not
          // discard an otherwise successful editor commit.
        }
      }
      set({
        entries: next,
        document3: { ...get().document3, dirty: false },
        editConflict: null,
        error: null,
        ...(leaveIssues.length > 0
          ? {
              issues: leaveIssues,
              windows: { ...get().windows, issues: true },
            }
          : {}),
      });
      return result.entry;
    } catch (error) {
      if (!isOptimisticLock(error)) throw error;
      const remote = await rpc<EntryDto>("entry.get", { index });
      set({
        editConflict: {
          index,
          key: { ...e.key },
          source: e.source,
          previous: e.translation,
          ours: translation,
          theirs: remote.translation,
          note,
          default_translation: defaultTranslation,
          remote_revision: remote.revision,
        },
        error: String(error),
      });
      throw error;
    }
  },
  commit: async (opts) => {
    const updated = await get().commitCurrent(opts);
    if (!updated) return;
    const { index, entries } = get();
    const ni = Math.min(index + 1, entries.length - 1);
    await get().select(ni);
  },
  save: async () => {
    await rpc("project.save");
    const root = get().props?.root ?? "";
    const d = get().document3;
    set({
      document3: { ...d, dirty: false },
      status: t("save"),
    });
    get().logLine(`saved TMX ${root}/omegat/project_save.tmx`);
    get().logLine(`Document3 range ${d.translationStart}-${d.translationEnd}`);
  },
  compile: async (file) => {
    try {
      await get().runLongOperation(
        "compile",
        file ? { file } : {},
      );
    } catch (error) {
      if (!isAbortError(error)) throw error;
      set({ status: "compile cancelled" });
      return;
    }
    set({ stats: await rpc<StatsDto>("stats.get") });
    const target = file ?? get().props?.target_dir ?? "";
    get().logLine(`compiled target ${target}`);
    const d = get().document3;
    get().logLine(`Document3 range ${d.translationStart}-${d.translationEnd}`);
  },
  jump: async (kind, n, dir = 1) => {
    const { entries, index } = get();
    if (entries.length === 0) return;
    const filter = makeFilter(
      get().editorFilter.kind,
      get().editorFilter.query,
    );
    const allowed = (entry: EntryDto) => filter.allowed({
      ...entry,
      translation: entry.translation,
    });
    const findCyclic = (pred: (entry: EntryDto) => boolean, step: 1 | -1) =>
      findCyclicEntryIndex(entries, index, step, allowed, pred) ?? -1;
    let next = -1;
    if (kind === "next") next = findCyclic(() => true, 1);
    else if (kind === "prev") next = findCyclic(() => true, -1);
    else if (kind === "number" && n != null) {
      const candidate = Math.max(0, Math.min(entries.length - 1, n - 1));
      if (allowed(entries[candidate]!)) next = candidate;
    } else if (kind === "untranslated") next = findCyclic((e) => !e.translated, dir);
    else if (kind === "translated") next = findCyclic((e) => e.translated, dir);
    else if (kind === "unique") {
      const counts = new Map<string, number>();
      entries.forEach((e) => counts.set(e.source, (counts.get(e.source) ?? 0) + 1));
      next = findCyclic((e) => (counts.get(e.source) ?? 0) === 1, dir);
    } else if (kind === "note") next = findCyclic((e) => Boolean(e.note), dir);
    else if (kind === "auto") {
      next = findCyclic((e) => e.properties.some(([k, v]) => k === "tm" && v === "auto"), dir);
    } else if (kind === "enforce") {
      next = findCyclic((e) => e.properties.some(([k, v]) => k === "tm" && v === "enforce"), dir);
    }
    if (next >= 0) await get().select(next);
  },
  historyBack: async () => {
    const back = get().navBack;
    const prev = back[back.length - 1];
    if (prev === undefined) return;
    const current = get().index;
    const forward = get().navForward;
    await get().select(prev, false);
    set({ navBack: back.slice(0, -1), navForward: [...forward, current] });
  },
  historyForward: async () => {
    const fwd = get().navForward;
    const next = fwd[fwd.length - 1];
    if (next === undefined) return;
    const current = get().index;
    const back = get().navBack;
    await get().select(next, false);
    set({ navForward: fwd.slice(0, -1), navBack: [...back, current] });
  },
  toggleMark: async (key) => {
    if (key === "modification") return;
    const marks = { ...get().marks, [key]: !get().marks[key] };
    set({ marks });
    await get().persistMarksAndLayout();
  },
  setModification: async (v) => {
    set({ marks: { ...get().marks, modification: v } });
    await get().persistMarksAndLayout();
  },
  setLayout: (partial) => {
    const layout = { ...get().layout, ...partial };
    set({ layout });
    writeLocal("omegat.layout", serializeDockLayout(layout));
    void get().persistMarksAndLayout();
  },
  restoreLayout: () => {
    set({ layout: { ...DEFAULT_DOCK_LAYOUT } });
    writeLocal("omegat.layout", serializeDockLayout(DEFAULT_DOCK_LAYOUT));
    void get().persistMarksAndLayout();
  },
  persistMarksAndLayout: async () => {
    const prefs = get().prefs;
    const docking = layoutToPrefs(get().layout);
    writeLocal("omegat.layout", serializeDockLayout(get().layout));
    if (!prefs) return;
    await get().savePrefs(
      defaultPreferences({
        ...prefs,
        marks: prefsFromMarks(get().marks),
        docking_layout: docking,
        filter_untranslated: get().filterUntranslated,
        completer_auto: get().completerAuto,
        history_completion: get().historyCompletion,
        history_prediction: get().historyPrediction,
        mt_auto_fetch: get().mtAutoFetch,
      }),
    );
  },
  toggleTheme: () => {
    const theme = get().theme === "light" ? "dark" : "light";
    set({ theme });
    if (typeof document !== "undefined") document.documentElement.dataset.theme = theme;
    const prefs = get().prefs;
    if (prefs) void get().savePrefs({ ...prefs, theme });
  },
}));

projectEvents.subscribe((projectEvent) => {
  useApp.setState({ projectEvent });
});

export function connectRpcOperationEvents(): () => void {
  return window.omegat?.onRpcOperation?.((event) => {
    useApp.setState((state) => ({
      longOperation: applyRpcOperationEvent(state.longOperation, event),
    }));
  }) ?? (() => undefined);
}

export function connectExternalProjectEvents(): () => void {
  let observedProject = "";
  const observedFingerprints = new Map<
    string,
    { fingerprint: string | null; sources: Set<"native" | "sidecar"> }
  >();
  const pending: Array<{
    root: string;
    generation: number;
    paths: string[];
    sources: Array<"native" | "sidecar">;
  }> = [];
  let draining = false;

  const drain = async () => {
    if (draining) return;
    draining = true;
    try {
      while (pending.length > 0) {
        const batch = pending.shift()!;
        const state = useApp.getState();
        if (
          state.props?.root !== batch.root
          || state.projectEvent.projectGeneration !== batch.generation
        ) {
          continue;
        }
        try {
          await state.refreshEntriesAfterExternalChange(undefined, true);
          const current = useApp.getState();
          if (
            current.props?.root === batch.root
            && current.projectEvent.projectGeneration === batch.generation
          ) {
            current.logLine(
              `external project refresh (${
                batch.paths.length
              } path(s), ${batch.sources.join("+")})`,
            );
          }
        } catch (error) {
          const current = useApp.getState();
          if (
            current.props?.root === batch.root
            && current.projectEvent.projectGeneration === batch.generation
          ) {
            useApp.setState({ error: String(error) });
          }
        }
      }
    } finally {
      draining = false;
      if (pending.length > 0) void drain();
    }
  };

  return window.omegat?.onProjectExternalChange?.(({
    root,
    paths,
    fingerprints,
    generation,
    sources,
  }) => {
    const state = useApp.getState();
    if (
      state.props?.root !== root
      || state.projectEvent.projectGeneration !== generation
    ) return;
    const project = `${generation}\0${root}`;
    if (observedProject !== project) {
      observedProject = project;
      observedFingerprints.clear();
    }
    const repeatedPaths = paths.filter((path) =>
      observedFingerprints.get(path)?.fingerprint === (fingerprints[path] ?? null)
    );
    const changedPaths: string[] = [];
    paths.forEach((path) => {
      const fingerprint = fingerprints[path] ?? null;
      const observed = observedFingerprints.get(path);
      if (observed?.fingerprint === fingerprint) {
        sources.forEach((source) => observed.sources.add(source));
      } else {
        const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "");
        const isRepeatedPathParent = fingerprint === null
          && repeatedPaths.some((repeated) => {
            const child = repeated.replaceAll("\\", "/");
            return child.startsWith(`${normalized}/`);
          });
        if (!isRepeatedPathParent) changedPaths.push(path);
        observedFingerprints.set(path, {
          fingerprint,
          sources: new Set(sources),
        });
      }
    });
    if (changedPaths.length === 0) {
      state.logLine(
        `coalesced external project change (${paths.length} path(s), ${sources.join("+")})`,
      );
      return;
    }
    pending.push({
      root,
      generation,
      paths: changedPaths,
      sources: [...sources],
    });
    void drain();
  }) ?? (() => undefined);
}

export function resetAppState() {
  nextLongOperationId = 1;
  projectEvents.reset();
  useApp.setState({
    ...initialState,
    projectEvent: projectEvents.current(),
    firstRun: true,
    locale: "en",
    windows: {},
  });
}

export { t };
