import { beforeEach, describe, expect, it, vi } from "vitest";
import { allowInsert } from "../editor/DocumentFilter3";
import { defaultPreferences } from "../lib/preferences";
import { resetAppState, useApp } from "../store/app";
import { dispatchMenuAction } from "./actions";

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
      pickDir: async () => "/proj",
      pickFile: async () => null,
      pickFiles: async () => [],
      saveText: async () => "selection.txt",
      quit: async () => undefined,
      relaunch: async () => undefined,
      openPath: async () => undefined,
      openExternal: async () => undefined,
      onMenu: () => () => undefined,
      setMenuLocale: async () => undefined,
    },
    confirm: () => true,
    prompt: () => "1",
  });
}

type E = {
  index: number;
  file: string;
  id: string;
  source: string;
  translation: string;
  note: string;
  comment: string;
  default_translation: boolean;
  revision: number;
  translated: boolean;
  tags: string[];
  properties: [string, string][];
};

function entry(index: number, source: string): E {
  return {
    index,
    file: "source.txt",
    id: String(index + 1),
    source,
    translation: "",
    note: "",
    comment: "",
    default_translation: true,
    revision: 1,
    translated: false,
    tags: source.includes("<x0/>") ? ["<x0/>"] : [],
    properties: [],
  };
}

describe("P8 keyboard walkthrough", () => {
  beforeEach(() => {
    rpc.mockReset();
    installBridge();
    resetAppState();
  });

  it("new → translate 3 (tags intact) → save → compile → replace → marks persist after reload", async () => {
    const log: string[] = [];
    let entries = [entry(0, "One <x0/> two"), entry(1, "Second"), entry(2, "Third")];
    let prefs = defaultPreferences({ marks: { ...defaultPreferences().marks, translated: true } });
    const props = {
      root: "/proj",
      source_lang: "en",
      target_lang: "fr",
      sentence_seg: true,
      source_dir: "/proj/source",
      target_dir: "/proj/target",
      glossary_dir: "/proj/glossary",
      tm_dir: "/proj/tm",
      export_tm_dir: "/proj/omegat",
    };

    rpc.mockImplementation(async (method: string, params?: Record<string, unknown>) => {
      if (method === "project.create") {
        log.push("create");
        return {};
      }
      if (method === "project.open") {
        log.push("open");
        return props;
      }
      if (method === "entry.list") return entries;
      if (method === "entry.set") {
        const i = Number(params?.index ?? 0);
        const translation = String(params?.translation ?? "");
        const e = {
          ...entries[i]!,
          translation,
          translated: translation.length > 0,
          revision: entries[i]!.revision + 1,
          note: String(params?.note ?? ""),
        };
        entries = entries.map((x, j) => (j === i ? e : x));
        log.push(`commit:${i}:${translation}`);
        return e;
      }
      if (method === "project.save") {
        log.push("save");
        return {};
      }
      if (method === "project.compile") {
        log.push("compile");
        return {};
      }
      if (method === "search.replace") {
        log.push(`replace:${params?.query}->${params?.replace}`);
        entries = entries.map((e) =>
          e.translation.includes(String(params?.query))
            ? { ...e, translation: e.translation.replace(String(params?.query), String(params?.replace)) }
            : e,
        );
        return { replaced: 1 };
      }
      if (method === "search.run") return [];
      if (method === "prefs.get") return prefs;
      if (method === "prefs.set") {
        prefs = defaultPreferences(params as typeof prefs);
        log.push(`prefs.translated=${prefs.marks.translated}`);
        return prefs;
      }
      if (method === "matches.query") return [];
      if (method === "glossary.query") return [];
      if (method === "issues.list") return [];
      if (method === "stats.get") return { segments: entries.length, translated: entries.filter((e) => e.translated).length };
      if (method === "completer.query") return [];
      return {};
    });

    expect(allowInsert("One <x0/> two", 6)).toBe(false);
    expect(allowInsert("One <x0/> two", 3)).toBe(true);

    await useApp.getState().create("/proj", "en", "fr", true);
    useApp.getState().setDraft("Un <x0/> deux");
    await useApp.getState().commit();
    useApp.getState().setDraft("Deuxieme");
    await useApp.getState().commit();
    useApp.getState().setDraft("Troisieme");
    await useApp.getState().commit();
    await dispatchMenuAction("project.save");
    await dispatchMenuAction("project.compile");
    useApp.getState().setSearchForm({ query: "Un", replace: "Une", translation: true });
    await useApp.getState().replaceAll();
    const translatedBefore = useApp.getState().marks.translated;
    await useApp.getState().toggleMark("translated");

    const savedPrefs = prefs;
    resetAppState();
    installBridge();
    useApp.getState().applyPrefs(savedPrefs);

    expect(useApp.getState().marks.translated).toBe(!translatedBefore);
    expect(entries[0]!.translation.includes("<x0/>")).toBe(true);
    expect(log).toEqual([
      "create",
      "open",
      "prefs.translated=true",
      "commit:0:Un <x0/> deux",
      "commit:1:Deuxieme",
      "commit:2:Troisieme",
      "save",
      "compile",
      "replace:Un->Une",
      `prefs.translated=${!translatedBefore}`,
    ]);
  });
});
