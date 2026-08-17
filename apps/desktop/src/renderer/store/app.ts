import { create } from "zustand";
import type {
  CompleterItemDto,
  DictHitDto,
  EntryDto,
  FilterInfoDto,
  GlossaryHitDto,
  IssueDto,
  MatchDto,
  MtSuggestionDto,
  Preferences,
  ProjectPropsDto,
  StatsDto,
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

type Screen = "welcome" | "workspace";

type State = {
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
  note: string;
  query: string;
  firstRun: boolean;
  locale: string;
  mt: MtSuggestionDto[];
  dict: DictHitDto[];
  completer: CompleterItemDto[];
  filters: FilterInfoDto[];
  prefs: Preferences | null;
  teamMessage: string;
  undoStack: string[];
  undo: () => void;
  setLocale: (locale: string) => void;
  queryMt: () => Promise<void>;
  queryDict: (word: string) => Promise<void>;
  queryCompleter: (prefix: string) => Promise<void>;
  loadFilters: () => Promise<void>;
  loadPrefs: () => Promise<void>;
  savePrefs: (p: Preferences) => Promise<void>;
  replaceAll: (query: string, replace: string, regex: boolean) => Promise<number>;
  teamSync: () => Promise<void>;
  learnWord: (word: string) => Promise<void>;
  ignoreWord: (word: string) => Promise<void>;
  loadVersion: () => Promise<void>;
  open: (root: string) => Promise<void>;
  create: (root: string, sl: string, tl: string, seg: boolean) => Promise<void>;
  select: (index: number) => Promise<void>;
  setDraft: (v: string) => void;
  commit: () => Promise<void>;
  save: () => Promise<void>;
  compile: () => Promise<void>;
  insertBest: () => void;
  toggleTheme: () => void;
};

export const useApp = create<State>((set, get) => ({
  screen: "welcome",
  version: "",
  props: null,
  entries: [],
  index: 0,
  matches: [],
  glossary: [],
  stats: null,
  issues: [],
  theme: "light",
  error: null,
  draft: "",
  note: "",
  query: "",
  mt: [],
  dict: [],
  completer: [],
  filters: [],
  prefs: null,
  teamMessage: "",
  undoStack: [],
  firstRun: !readLocal("omegat.first"),
  locale: (() => {
    const saved = readLocal("omegat.locale");
    const nav = typeof navigator !== "undefined" ? navigator.language : "en";
    const loc = detectLocale(saved || nav);
    applyDocumentLocale(loc);
    return loc;
  })(),
  setLocale: (locale) => {
    applyDocumentLocale(locale);
    writeLocal("omegat.locale", locale);
    set({ locale });
  },
  loadVersion: async () => {
    try {
      const v = await rpc<{ version: string }>("sys.version");
      set({ version: v.version });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  open: async (root) => {
    const props = await rpc<ProjectPropsDto>("project.open", { root });
    const entries = await rpc<EntryDto[]>("entry.list");
    const stats = await rpc<StatsDto>("stats.get");
    set({ props, entries, screen: "workspace", index: 0 });
    set({ stats });
    await get().select(0);
    const rec = JSON.parse(readLocal("omegat.recent") || "[]") as string[];
    writeLocal("omegat.recent", JSON.stringify([root, ...rec.filter((r) => r !== root)].slice(0, 8)));
    writeLocal("omegat.first", "1");
    set({ firstRun: false });
  },
  create: async (root, sl, tl, seg) => {
    await rpc("project.create", { root, source_lang: sl, target_lang: tl, sentence_seg: seg });
    await get().open(root);
  },
  select: async (index) => {
    const { entries } = get();
    const e = entries[index];
    if (!e) return;
    const matches = await rpc<MatchDto[]>("matches.query", { index });
    const glossary = await rpc<GlossaryHitDto[]>("glossary.query", { index });
    const issues = await rpc<IssueDto[]>("issues.list");
    let mt: MtSuggestionDto[] = [];
    try {
      const one = await rpc<MtSuggestionDto>("mt.query", { index, engine: "mymemory" });
      mt = [one];
    } catch {
      mt = [];
    }
    const dict = await rpc<DictHitDto[]>("dict.query", { word: e.source.split(/\s+/)[0] || "" });
    const completer = await rpc<CompleterItemDto[]>("completer.query", { index, prefix: "" });
    set({
      index,
      matches,
      glossary,
      issues,
      mt,
      dict,
      completer,
      draft: e.translation,
      note: e.note,
    });
  },
  queryMt: async () => {
    const { index } = get();
    try {
      const one = await rpc<MtSuggestionDto>("mt.query", { index, engine: "mymemory" });
      set({ mt: [one] });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  queryDict: async (word) => {
    const dict = await rpc<DictHitDto[]>("dict.query", { word });
    set({ dict });
  },
  queryCompleter: async (prefix) => {
    const completer = await rpc<CompleterItemDto[]>("completer.query", { index: get().index, prefix });
    set({ completer });
  },
  loadFilters: async () => {
    const filters = await rpc<FilterInfoDto[]>("filters.list");
    set({ filters });
  },
  loadPrefs: async () => {
    const prefs = await rpc<Preferences>("prefs.get");
    set({ prefs });
  },
  savePrefs: async (p) => {
    const prefs = await rpc<Preferences>("prefs.set", p);
    set({ prefs });
  },
  replaceAll: async (query, replace, regex) => {
    const r = await rpc<{ replaced: number }>("search.replace", {
      query,
      replace,
      regex,
      source: false,
      translation: true,
    });
    const entries = await rpc<EntryDto[]>("entry.list");
    set({ entries });
    return r.replaced;
  },
  teamSync: async () => {
    try {
      const r = await rpc<{ action: string; message: string }>("team.sync");
      set({ teamMessage: `${r.action}: ${r.message}` });
    } catch (e) {
      set({ teamMessage: String(e), error: String(e) });
    }
  },
  learnWord: async (word) => {
    await rpc("spell.learn", { word });
  },
  ignoreWord: async (word) => {
    await rpc("spell.ignore", { word });
  },
  setDraft: (v) => {
    const prev = get().draft;
    set({ draft: v, undoStack: [...get().undoStack.slice(-49), prev] });
  },
  undo: () => {
    const stack = get().undoStack;
    const last = stack[stack.length - 1];
    if (last === undefined) return;
    set({ draft: last, undoStack: stack.slice(0, -1) });
  },
  commit: async () => {
    const { index, entries, draft, note } = get();
    const e = entries[index];
    if (!e) return;
    const updated = await rpc<EntryDto>("entry.set", {
      index,
      translation: draft,
      note,
      revision: e.revision,
      default_translation: true,
    });
    const next = entries.map((x, i) => (i === index ? updated : x));
    set({ entries: next });
    const ni = Math.min(index + 1, next.length - 1);
    await get().select(ni);
  },
  save: async () => {
    await rpc("project.save");
  },
  compile: async () => {
    await rpc("project.compile");
    const stats = await rpc<StatsDto>("stats.get");
    set({ stats });
  },
  insertBest: () => {
    const m = get().matches[0];
    if (m) set({ draft: m.translation });
  },
  toggleTheme: () => {
    const theme = get().theme === "light" ? "dark" : "light";
    set({ theme });
    document.documentElement.dataset.theme = theme;
  },
}));

export { t };
