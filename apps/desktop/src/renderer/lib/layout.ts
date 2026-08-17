/** Semantic copy of Java `DockingDefaults.xml` (orientation/location only). */
export type DockLayout = {
  left: number;
  notes: number;
  editorStack: number;
  editorMain: number;
  props: number;
  matches: number;
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

export function layoutFromPrefs(extra: Record<string, string> | undefined, fallback?: string | null): DockLayout {
  return parseDockLayout(extra?.docking_layout ?? extra?.MAINWINDOW_LAYOUT ?? fallback ?? null);
}

export function isDockLayout(value: unknown): value is DockLayout {
  if (!value || typeof value !== "object") return false;
  return KEYS.every((k) => k in (value as object));
}
