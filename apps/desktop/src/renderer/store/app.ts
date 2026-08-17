import { create } from "zustand";
import type {
  EntryDto,
  GlossaryHitDto,
  IssueDto,
  MatchDto,
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
  setLocale: (locale: string) => void;
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
    set({
      index,
      matches,
      glossary,
      issues,
      draft: e.translation,
      note: e.note,
    });
  },
  setDraft: (v) => set({ draft: v }),
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
