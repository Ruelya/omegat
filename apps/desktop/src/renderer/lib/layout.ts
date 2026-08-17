/** Semantic copy of Java `DockingDefaults.xml` (orientation/location only). */
import type { DockingLayoutPrefs } from "./types";

export type DockLayout = {
  left: number;
  notes: number;
  editorStack: number;
  editorMain: number;
  props: number;
  matches: number;
  east: number;
  dictMt: number;
  showDict: boolean;
  showMt: boolean;
};

export const DEFAULT_DOCK_LAYOUT: DockLayout = {
  left: 0.25,
  notes: 0.2,
  editorStack: 0.65,
  editorMain: 0.75,
  props: 0.5,
  matches: 0.8,
  east: 0.78,
  dictMt: 0.5,
  showDict: true,
  showMt: true,
};

const KEYS: (keyof DockLayout)[] = [
  "left",
  "notes",
  "editorStack",
  "editorMain",
  "props",
  "matches",
  "east",
  "dictMt",
  "showDict",
  "showMt",
];

export function clampRatio(n: number, min = 0.12, max = 0.88): number {
  if (Number.isNaN(n)) return min;
  return Math.min(max, Math.max(min, n));
}

export function normalizeDockLayout(partial: Partial<DockLayout> | null | undefined): DockLayout {
  const src = { ...DEFAULT_DOCK_LAYOUT, ...(partial ?? {}) };
  return {
    left: clampRatio(src.left),
    notes: clampRatio(src.notes),
    editorStack: clampRatio(src.editorStack),
    editorMain: clampRatio(src.editorMain),
    props: clampRatio(src.props),
    matches: clampRatio(src.matches),
    east: clampRatio(src.east, 0.45, 0.92),
    dictMt: clampRatio(src.dictMt),
    showDict: Boolean(src.showDict),
    showMt: Boolean(src.showMt),
  };
}

export function serializeDockLayout(layout: DockLayout): string {
  return JSON.stringify(normalizeDockLayout(layout));
}

export function parseDockLayout(raw: string | null | undefined): DockLayout {
  if (!raw) return { ...DEFAULT_DOCK_LAYOUT };
  try {
    const parsed = JSON.parse(raw) as Partial<DockLayout>;
    if (!parsed || typeof parsed !== "object") return { ...DEFAULT_DOCK_LAYOUT };
    return normalizeDockLayout(parsed);
  } catch {
    return { ...DEFAULT_DOCK_LAYOUT };
  }
}

export function layoutFromPrefs(dock: DockingLayoutPrefs | undefined, fallback?: string | null): DockLayout {
  if (dock) {
    return normalizeDockLayout({
      left: dock.left,
      notes: dock.notes,
      editorStack: dock.editor_stack,
      editorMain: dock.editor_main,
      props: dock.props,
      matches: dock.matches,
      east: dock.east,
      dictMt: dock.dict_mt,
      showDict: dock.show_dict,
      showMt: dock.show_mt,
    });
  }
  return parseDockLayout(fallback ?? null);
}

export function layoutToPrefs(layout: DockLayout): DockingLayoutPrefs {
  const n = normalizeDockLayout(layout);
  return {
    left: n.left,
    notes: n.notes,
    editor_stack: n.editorStack,
    editor_main: n.editorMain,
    props: n.props,
    matches: n.matches,
    east: n.east,
    dict_mt: n.dictMt,
    show_dict: n.showDict,
    show_mt: n.showMt,
  };
}

export function isDockLayout(value: unknown): value is DockLayout {
  if (!value || typeof value !== "object") return false;
  return KEYS.every((k) => k in (value as object));
}
