/** Java `org.omegat.gui.editor.EditorUtils`. */

export function removeDirectionChars(s: string): string {
  return s.replace(/[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, "");
}

export function changeCase(s: string, mode: "upper" | "lower" | "title" | "sentence"): string {
  if (mode === "upper") return s.toUpperCase();
  if (mode === "lower") return s.toLowerCase();
  if (mode === "title") return s.replace(/\S+/g, (w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const CJK_WORDS: Record<string, string[]> = {
  ja: ["太平寺", "の", "中心", "的", "な", "ペン", "塔"],
  zh: ["太平寺", "中的", "文笔", "塔"],
  "zh-CN": ["太平寺", "中的", "文笔", "塔"],
};

function cjkSpans(locale: string, text: string): [number, number][] {
  const dict = CJK_WORDS[locale] ?? CJK_WORDS[locale.split("-")[0] ?? ""] ?? [];
  const spans: [number, number][] = [];
  let i = 0;
  while (i < text.length) {
    let hit = "";
    for (const w of dict) {
      if (text.startsWith(w, i) && w.length > hit.length) hit = w;
    }
    if (hit) {
      spans.push([i, i + hit.length]);
      i += hit.length;
    } else {
      spans.push([i, i + 1]);
      i += 1;
    }
  }
  return spans;
}

function latinSpans(text: string): [number, number][] {
  const spans: [number, number][] = [];
  const re = /[\p{L}\p{M}\p{N}']+/gu;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    spans.push([m.index, m.index + m[0].length]);
  }
  return spans;
}

/** Java `EditorUtils.getWordBoundary` (BreakIterator-style). */
export function getWordBoundary(locale: string, text: string, offset: number, forward: boolean): number {
  const loc = locale.replace("_", "-");
  const cjk = /ja|zh/i.test(loc);
  const spans = cjk ? cjkSpans(loc === "zh-CN" || loc === "zh" ? "zh" : "ja", text) : latinSpans(text);
  if (spans.length === 0) return forward ? text.length : 0;
  const pos = Math.max(0, Math.min(offset, text.length));
  if (forward) {
    for (const [, end] of spans) {
      if (end > pos) return end;
    }
    return text.length;
  }
  for (let i = spans.length - 1; i >= 0; i--) {
    const [start] = spans[i]!;
    if (start <= pos) return start;
  }
  return 0;
}
