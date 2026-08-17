export type DocToken = { kind: "text" | "tag"; value: string };

const TAG_RE = /<\/?(?:f|x|g|ex|bx|ph|it|bpt|ept|hi|sub)\d*\/?>|<\/?[A-Za-z][\w:-]*\d*\/?>/g;

export function parseDocument(text: string): DocToken[] {
  const tokens: DocToken[] = [];
  let last = 0;
  const re = new RegExp(TAG_RE.source, "g");
  for (const m of text.matchAll(re)) {
    const i = m.index ?? 0;
    if (i > last) tokens.push({ kind: "text", value: text.slice(last, i) });
    tokens.push({ kind: "tag", value: m[0] });
    last = i + m[0].length;
  }
  if (last < text.length) tokens.push({ kind: "text", value: text.slice(last) });
  if (tokens.length === 0 && text) tokens.push({ kind: "text", value: text });
  return tokens;
}

export function extractTags(text: string): string[] {
  return parseDocument(text).filter((t) => t.kind === "tag").map((t) => t.value);
}

export function nextMissingTag(source: string, target: string): string | null {
  const src = extractTags(source);
  const used = new Map<string, number>();
  for (const t of extractTags(target)) used.set(t, (used.get(t) ?? 0) + 1);
  const seen = new Map<string, number>();
  for (const t of src) {
    const n = (seen.get(t) ?? 0) + 1;
    seen.set(t, n);
    if ((used.get(t) ?? 0) < n) return t;
  }
  return null;
}

export function insertAt(text: string, insertion: string, offset = text.length): string {
  const i = Math.max(0, Math.min(offset, text.length));
  return text.slice(0, i) + insertion + text.slice(i);
}

export function serializeFromElement(root: HTMLElement): string {
  let out = "";
  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      out += node.textContent ?? "";
      return;
    }
    if (node instanceof HTMLElement) {
      if (node.dataset.tag) {
        out += node.dataset.tag;
        return;
      }
      node.childNodes.forEach(walk);
    }
  };
  root.childNodes.forEach(walk);
  return out.replace(/\u00a0/g, "\u00a0");
}

export type ViewMarks = {
  whitespace: boolean;
  nbsp: boolean;
  bidi: boolean;
  glossary: boolean;
  translated: boolean;
  untranslated: boolean;
  noted: boolean;
  nonUnique: boolean;
  autoPopulated: boolean;
  alternative: boolean;
  paragraphStart: boolean;
  displaySource: boolean;
  languageChecker: boolean;
  fontFallback: boolean;
  modification: "none" | "selected" | "all";
};

export const DEFAULT_MARKS: ViewMarks = {
  whitespace: false,
  nbsp: false,
  bidi: false,
  glossary: true,
  translated: true,
  untranslated: true,
  noted: true,
  nonUnique: false,
  autoPopulated: true,
  alternative: true,
  paragraphStart: false,
  displaySource: true,
  languageChecker: false,
  fontFallback: false,
  modification: "none",
};

const MARK_KEYS: Record<keyof Omit<ViewMarks, "modification">, string> = {
  whitespace: "mark_whitespace",
  nbsp: "mark_nbsp",
  bidi: "mark_bidi",
  glossary: "mark_glossary_matches",
  translated: "mark_translated",
  untranslated: "mark_untranslated",
  noted: "mark_noted_segments",
  nonUnique: "mark_non_unique",
  autoPopulated: "mark_auto_populated",
  alternative: "mark_alternative",
  paragraphStart: "mark_paragraph_start",
  displaySource: "display_segment_source",
  languageChecker: "mark_language_checker",
  fontFallback: "mark_font_fallback",
};

export function marksFromExtra(extra: Record<string, string> | undefined): ViewMarks {
  const m = { ...DEFAULT_MARKS };
  if (!extra) return m;
  (Object.keys(MARK_KEYS) as (keyof typeof MARK_KEYS)[]).forEach((k) => {
    const raw = extra[MARK_KEYS[k]];
    if (raw === "true") m[k] = true;
    if (raw === "false") m[k] = false;
  });
  const info = extra.display_modification_info;
  if (info === "selected" || info === "all" || info === "none") m.modification = info;
  return m;
}

export function extraFromMarks(marks: ViewMarks): Record<string, string> {
  const extra: Record<string, string> = {};
  (Object.keys(MARK_KEYS) as (keyof typeof MARK_KEYS)[]).forEach((k) => {
    extra[MARK_KEYS[k]] = String(marks[k]);
  });
  extra.display_modification_info = marks.modification;
  return extra;
}

export type MarkSpan = { text: string; cls: string[] };

export function decorateText(text: string, marks: ViewMarks, glossary: string[] = []): MarkSpan[] {
  const spans: MarkSpan[] = [];
  const terms = glossary.filter(Boolean).sort((a, b) => b.length - a.length);
  let i = 0;
  while (i < text.length) {
    const ch = text[i]!;
    if (marks.nbsp && ch === "\u00a0") {
      spans.push({ text: "⍽", cls: ["mark-nbsp"] });
      i += 1;
      continue;
    }
    if (marks.bidi && "\u200e\u200f\u202a\u202b\u202c".includes(ch)) {
      spans.push({ text: ch === "\u200e" ? "LRM" : ch === "\u200f" ? "RLM" : "BIDI", cls: ["mark-bidi"] });
      i += 1;
      continue;
    }
    if (marks.whitespace && (ch === " " || ch === "\t" || ch === "\n")) {
      spans.push({
        text: ch === " " ? "·" : ch === "\t" ? "→" : "¶\n",
        cls: ["mark-ws"],
      });
      i += 1;
      continue;
    }
    if (marks.glossary && terms.length) {
      const rest = text.slice(i);
      const hit = terms.find((t) => rest.toLowerCase().startsWith(t.toLowerCase()));
      if (hit) {
        spans.push({ text: text.slice(i, i + hit.length), cls: ["mark-glossary"] });
        i += hit.length;
        continue;
      }
    }
    spans.push({ text: ch, cls: [] });
    i += 1;
  }
  return mergeSpans(spans);
}

function mergeSpans(spans: MarkSpan[]): MarkSpan[] {
  const out: MarkSpan[] = [];
  for (const s of spans) {
    const last = out[out.length - 1];
    if (last && last.cls.join() === s.cls.join()) last.text += s.text;
    else out.push({ ...s });
  }
  return out;
}

export type HistoryStacks = { undo: string[]; redo: string[] };

export function pushUndo(stacks: HistoryStacks, prev: string, next: string): HistoryStacks {
  if (prev === next) return stacks;
  return { undo: [...stacks.undo.slice(-99), prev], redo: [] };
}

export function undoDraft(stacks: HistoryStacks, current: string): { draft: string; stacks: HistoryStacks } {
  const prev = stacks.undo[stacks.undo.length - 1];
  if (prev === undefined) return { draft: current, stacks };
  return {
    draft: prev,
    stacks: { undo: stacks.undo.slice(0, -1), redo: [...stacks.redo, current] },
  };
}

export function redoDraft(stacks: HistoryStacks, current: string): { draft: string; stacks: HistoryStacks } {
  const next = stacks.redo[stacks.redo.length - 1];
  if (next === undefined) return { draft: current, stacks };
  return {
    draft: next,
    stacks: { undo: [...stacks.undo, current], redo: stacks.redo.slice(0, -1) },
  };
}

export function switchCase(text: string, mode: "upper" | "lower" | "title" | "sentence" | "cycle"): string {
  if (mode === "upper") return text.toUpperCase();
  if (mode === "lower") return text.toLowerCase();
  if (mode === "title") {
    return text.replace(/\w\S*/g, (w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
  }
  if (mode === "sentence") {
    return text.replace(/(^\s*[a-z])|([.!?]\s+[a-z])/g, (m) => m.toUpperCase());
  }
  if (text === text.toUpperCase()) return text.toLowerCase();
  if (text === text.toLowerCase()) {
    return text.replace(/\w\S*/g, (w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
  }
  return text.toUpperCase();
}
