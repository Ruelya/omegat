import { create } from "zustand";
import { createDocument3, replaceEditText, type Document3State } from "../editor/Document3";
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
import { applyColorVars, defaultPreferences } from "../lib/preferences";
import { defaultSearchForm, persistSearchForm, restoreSearchForm, type SearchForm } from "../lib/search-params";
import type {
  CompleterItemDto,
  DictHitDto,
  EditorConflict,
  EntryDto,
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

async function rpc<T>(method: string, params?: unknown): Promise<T> {
  if (!window.omegat) {
    throw new Error("sidecar bridge unavailable");
  }
  return window.omegat.rpc(method, params) as Promise<T>;
}

function normalizeEntrySetResult(result: EntrySetResult | EntryDto): EntrySetResult {
  if ("entry" in result && Array.isArray(result.updated)) return result;
  return { entry: result, updated: [result] };
}

function isOptimisticLock(error: unknown): boolean {
  return /optimistic(?: lock| revision)/i.test(String(error));
}

type Screen = "welcome" | "workspace";

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
  draft: string;
  document3: Document3State;
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
  applyPrefs: (p: Preferences) => void;
  setLocale: (locale: string) => void;
  logLine: (line: string) => void;
  openWindow: (id: WindowId, open?: boolean) => void;
  loadVersion: () => Promise<void>;
  open: (root: string) => Promise<void>;
  create: (root: string, sl: string, tl: string, seg: boolean) => Promise<void>;
  closeProject: () => Promise<void>;
  reloadProject: () => Promise<void>;
  select: (index: number, recordHistory?: boolean) => Promise<void>;
  setDraft: (v: string) => void;
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
  resolveConflict: (side: "ours" | "theirs" | "manual", source?: string, translation?: string) => Promise<void>;
  resolveEditConflict: (side: "ours" | "theirs" | "manual", translation?: string) => Promise<void>;
  learnWord: (word: string) => Promise<void>;
  ignoreWord: (word: string) => Promise<void>;
  addGlossary: (source: string, target: string, comment?: string) => Promise<void>;
  importWiki: (source: string) => Promise<void>;
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
  draft: "",
  document3: createDocument3("", ""),
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
};

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
    const props = await rpc<ProjectPropsDto>("project.open", { root });
    const entries = await rpc<EntryDto[]>("entry.list");
    const stats = await rpc<StatsDto>("stats.get");
    set({ props, entries, screen: "workspace", index: 0, stats, error: null });
    await get().loadPrefs();
    await get().select(0, false);
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
    await rpc("project.create", { root, source_lang: sl, target_lang: tl, sentence_seg: seg });
    await get().open(root);
  },
  closeProject: async () => {
    try {
      await rpc("project.close");
    } catch {
      /* ignore */
    }
    set({ ...initialState, locale: get().locale, theme: get().theme, version: get().version, firstRun: false, screen: "welcome" });
  },
  reloadProject: async () => {
    const root = get().props?.root;
    if (!root) return;
    await rpc("project.reload");
    const entries = await rpc<EntryDto[]>("entry.list");
    const stats = await rpc<StatsDto>("stats.get");
    set({ entries, stats });
    await get().select(get().index, false);
    get().logLine("reloaded project");
  },
  select: async (index, recordHistory = true) => {
    const before = get();
    const previousEntry = before.entries[before.index];
    if (
      previousEntry
      && (before.draft !== previousEntry.translation || before.note !== previousEntry.note)
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
    } = get();
    const insert_best = prefs?.insert_best_match ?? true;
    const e = entries[index];
    if (!e) return;
    const matches = await rpc<MatchDto[]>("matches.query", { index });
    const glossary = await rpc<GlossaryHitDto[]>("glossary.query", { index });
    const issues = await rpc<IssueDto[]>("issues.list");
    let mt: MtSuggestionDto[] = [];
    if (get().mtAutoFetch) {
      try {
        mt = [await rpc<MtSuggestionDto>("mt.query", { index, engine: "mymemory" })];
      } catch {
        mt = [];
      }
    }
    const dict = get().prefs?.dictionary_auto_search
      ? await rpc<DictHitDto[]>("dict.query", {
          word: e.source.split(/\s+/)[0] || "",
          fuzzy: get().prefs?.dictionary_fuzzy_matching,
        })
      : [];
    let draft = e.translation;
    if (!draft && insert_best && matches[0]) draft = matches[0].translation;
    const completer = get().completerAuto
      ? await rpc<CompleterItemDto[]>("completer.query", { index, prefix: "", text: draft })
      : [];
    set({
      index,
      matches,
      glossary,
      issues,
      mt,
      dict,
      completer,
      draft,
      document3: createDocument3(e.source, draft),
      note: e.note,
      history: { undo: [], redo: [] },
      selectedMatch: 0,
      status: `${e.file} #${index + 1}`,
      ...(recordHistory && prev !== index
        ? { navBack: [...navBack, prev], navForward: [] }
        : {}),
    });
  },
  queryMt: async (engine = "mymemory") => {
    try {
      const one = await rpc<MtSuggestionDto>("mt.query", { index: get().index, engine });
      set({ mt: [one, ...get().mt.filter((m) => m.engine !== engine)] });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  queryDict: async (word) => {
    set({
      dict: await rpc<DictHitDto[]>("dict.query", {
        word,
        fuzzy: get().prefs?.dictionary_fuzzy_matching,
      }),
    });
  },
  queryCompleter: async (prefix) => {
    if (!get().completerAuto && !prefix) {
      set({ completer: [] });
      return;
    }
    set({
      completer: await rpc<CompleterItemDto[]>("completer.query", {
        index: get().index,
        prefix,
        text: get().draft,
      }),
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
    const hits = await rpc<SearchHitDto[]>("search.run", {
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
    });
    set({ searchHits: hits });
    if (get().prefs) {
      void get().patchPrefs({ search_window: persistSearchForm(form) });
    }
    return hits;
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
    const entries = await rpc<EntryDto[]>("entry.list");
    set({ entries });
    await get().select(get().index, false);
    get().logLine(`replaced ${r.replaced}`);
    return r.replaced;
  },
  teamSync: async () => {
    try {
      const r = await rpc<{ action: string; message: string }>("team.sync");
      set({ teamMessage: `${r.action}: ${r.message}` });
    } catch (e) {
      const msg = String(e);
      set({ teamMessage: msg, error: msg });
      try {
        const c = await rpc<{ conflicts: TeamConflict[] | string[] }>("team.conflicts");
        const list = Array.isArray(c.conflicts)
          ? c.conflicts.map((x) => (typeof x === "string" ? { message: x } : x))
          : [];
        set({ teamConflicts: list });
      } catch {
        /* ignore */
      }
    }
  },
  teamCommit: async (which) => {
    const r = await rpc<{ action: string; message: string }>("team.commit", { which });
    set({ teamMessage: `${r.action}: ${r.message}` });
    get().logLine(`commit ${which}`);
  },
  resolveConflict: async (side, source, translation) => {
    const src = source ?? get().teamConflicts[0]?.source ?? "";
    const r = await rpc<{ conflicts: TeamConflict[] }>("team.resolve", {
      source: src,
      side,
      translation,
    });
    set({
      teamConflicts: r.conflicts ?? [],
      teamMessage: `keep ${side}${src ? ` (${src})` : ""}`,
    });
    await get().patchPrefs({ team_conflict_resolution: side });
  },
  resolveEditConflict: async (side, translation) => {
    const conflict = get().editConflict;
    if (!conflict) return;
    const latest = await rpc<EntryDto[]>("entry.list");
    const remote = latest[conflict.index];
    if (!remote || remote.source !== conflict.source) {
      throw new Error("editor conflict entry is no longer available");
    }
    if (side === "theirs") {
      set({
        entries: latest,
        draft: remote.translation,
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
      draft: chosen,
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
    await get().select(get().index, false);
  },
  ignoreWord: async (word) => {
    await rpc("spell.ignore", { word });
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
  setDraft: (v) => {
    const prev = get().draft;
    const src = get().entries[get().index]?.source ?? get().document3.source;
    const current = get().document3;
    const document3 = current.source === src
      ? replaceEditText(current, v)
      : replaceEditText(createDocument3(src, v), v);
    set({
      draft: v,
      document3,
      history: pushUndo(get().history, prev, v),
    });
  },
  setNote: (v) => set({ note: v }),
  undo: () => {
    const { draft, stacks } = undoDraft(get().history, get().draft);
    const src = get().entries[get().index]?.source ?? get().document3.source;
    set({ draft, document3: createDocument3(src, draft), history: stacks });
  },
  redo: () => {
    const { draft, stacks } = redoDraft(get().history, get().draft);
    const src = get().entries[get().index]?.source ?? get().document3.source;
    set({ draft, document3: createDocument3(src, draft), history: stacks });
  },
  applyCase: (mode) => get().setDraft(switchCase(get().draft, mode)),
  insertMatch: (n = 1, mode = "overwrite") => {
    const m = get().matches[(n ?? 1) - 1] ?? get().matches[get().selectedMatch];
    if (!m) return;
    if (mode === "insert") get().setDraft(get().draft + m.translation);
    else get().setDraft(m.translation);
    set({ selectedMatch: Math.max(0, (n ?? 1) - 1) });
  },
  insertMt: (mode = "overwrite") => {
    const m = get().mt[0];
    if (!m) return;
    if (mode === "insert") get().setDraft(get().draft + m.text);
    else get().setDraft(m.text);
  },
  insertSource: (mode = "overwrite") => {
    const src = get().entries[get().index]?.source ?? "";
    if (mode === "insert") get().setDraft(get().draft + src);
    else get().setDraft(src);
  },
  insertTag: () => {
    const e = get().entries[get().index];
    const tag = e ? nextMissingTag(e.source, get().draft) : null;
    if (tag) get().setDraft(get().draft + tag);
  },
  insertAllTags: () => {
    const e = get().entries[get().index];
    if (!e) return;
    let draft = get().draft;
    let tag = nextMissingTag(e.source, draft);
    while (tag) {
      draft += tag;
      tag = nextMissingTag(e.source, draft);
    }
    get().setDraft(draft);
  },
  insertChar: (ch) => get().setDraft(get().draft + ch),
  selectSource: () => {
    const src = get().entries[get().index]?.source ?? "";
    set({ selectedText: src, focusPanel: "editor" });
  },
  exportSelection: async () => {
    const text = get().selectedText || get().draft;
    if (window.omegat?.saveText) {
      await window.omegat.saveText("selection.txt", text);
    }
    get().logLine(`exported selection (${text.length} chars)`);
  },
  importFiles: async () => {
    const files = (await window.omegat?.pickFiles?.()) ?? [];
    if (!files.length) return;
    await rpc("project.import", { files });
    await get().reloadProject();
    get().logLine(`imported ${files.length} file(s)`);
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
      const entries = await rpc<EntryDto[]>("entry.list");
      set({ entries });
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
    const { index, entries, draft, note } = get();
    const e = entries[index];
    if (!e) return null;
    const defaultTranslation = opts?.default_translation ?? e.default_translation;
    try {
      const response = await rpc<EntrySetResult | EntryDto>("entry.set", {
        index,
        translation: draft,
        note,
        revision: e.revision,
        default_translation: defaultTranslation,
      });
      const result = normalizeEntrySetResult(response);
      const updates = new Map(result.updated.map((entry) => [entry.index, entry]));
      updates.set(result.entry.index, result.entry);
      const next = entries.map((entry) => updates.get(entry.index) ?? entry);
      set({
        entries: next,
        document3: { ...get().document3, dirty: false },
        editConflict: null,
        error: null,
      });
      return result.entry;
    } catch (error) {
      if (!isOptimisticLock(error)) throw error;
      const remote = await rpc<EntryDto>("entry.get", { index });
      set({
        editConflict: {
          index,
          source: e.source,
          previous: e.translation,
          ours: draft,
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
    await rpc("project.compile", file ? { file } : {});
    set({ stats: await rpc<StatsDto>("stats.get") });
    const target = file ?? get().props?.target_dir ?? "";
    get().logLine(`compiled target ${target}`);
    const d = get().document3;
    get().logLine(`Document3 range ${d.translationStart}-${d.translationEnd}`);
  },
  jump: async (kind, n, dir = 1) => {
    const { entries, index } = get();
    if (entries.length === 0) return;
    const allowed = (entry: EntryDto) => !get().filterUntranslated || !entry.translated;
    const findCyclic = (pred: (entry: EntryDto) => boolean, step: 1 | -1) => {
      for (let distance = 1; distance <= entries.length; distance += 1) {
        const candidate = (index + step * distance + entries.length * 2) % entries.length;
        const entry = entries[candidate]!;
        if (allowed(entry) && pred(entry)) return candidate;
      }
      return -1;
    };
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

export function resetAppState() {
  useApp.setState({
    ...initialState,
    firstRun: true,
    locale: "en",
    windows: {},
  });
}

export { t };
