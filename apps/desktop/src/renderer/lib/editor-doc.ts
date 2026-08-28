import type { MarkPrefs } from "./types";
import { changeCase, type ChangeCaseMode } from "../editor/EditorUtils";

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

export function serializeTokens(tokens: DocToken[]): string {
  return tokens.map((t) => t.value).join("");
}

export type TokenSpan = { start: number; end: number; kind: "text" | "tag"; value: string };

export function tokenSpans(text: string): TokenSpan[] {
  const tokens = parseDocument(text);
  const spans: TokenSpan[] = [];
  let pos = 0;
  for (const tok of tokens) {
    spans.push({ start: pos, end: pos + tok.value.length, kind: tok.kind, value: tok.value });
    pos += tok.value.length;
  }
  return spans;
}

export function snapCaret(text: string, pos: number, bias: "before" | "after" = "after"): number {
  const n = Math.max(0, Math.min(pos, text.length));
  const span = tokenSpans(text).find((s) => s.start < n && n < s.end);
  if (!span || span.kind !== "tag") return n;
  return bias === "before" ? span.start : span.end;
}

export function expandToAtomic(text: string, start: number, end: number): { start: number; end: number } {
  let a = Math.max(0, Math.min(start, end));
  let b = Math.max(start, end);
  for (const s of tokenSpans(text)) {
    if (s.kind !== "tag") continue;
    const overlaps = a < s.end && b > s.start;
    if (overlaps) {
      a = Math.min(a, s.start);
      b = Math.max(b, s.end);
    }
  }
  return { start: a, end: b };
}

function previousCodePointBoundary(text: string, pos: number): number {
  let start = Math.max(0, pos - 1);
  const unit = text.charCodeAt(start);
  if (unit >= 0xdc00 && unit <= 0xdfff && start > 0) {
    const previous = text.charCodeAt(start - 1);
    if (previous >= 0xd800 && previous <= 0xdbff) start -= 1;
  }
  return start;
}

function nextCodePointBoundary(text: string, pos: number): number {
  const start = Math.max(0, Math.min(pos, text.length));
  const unit = text.charCodeAt(start);
  if (unit >= 0xd800 && unit <= 0xdbff && start + 1 < text.length) {
    const next = text.charCodeAt(start + 1);
    if (next >= 0xdc00 && next <= 0xdfff) return start + 2;
  }
  return Math.min(text.length, start + 1);
}

export function deleteBackwardAtomic(text: string, pos: number): { text: string; pos: number } {
  if (pos <= 0) return { text, pos: 0 };
  const prev = pos - 1;
  const span = tokenSpans(text).find((s) => s.start <= prev && prev < s.end);
  if (span?.kind === "tag") {
    return { text: text.slice(0, span.start) + text.slice(span.end), pos: span.start };
  }
  const start = previousCodePointBoundary(text, pos);
  return { text: text.slice(0, start) + text.slice(pos), pos: start };
}

export function deleteForwardAtomic(text: string, pos: number): { text: string; pos: number } {
  if (pos >= text.length) return { text, pos };
  const span = tokenSpans(text).find((s) => s.start <= pos && pos < s.end);
  if (span?.kind === "tag") {
    return { text: text.slice(0, span.start) + text.slice(span.end), pos: span.start };
  }
  return { text: text.slice(0, pos) + text.slice(nextCodePointBoundary(text, pos)), pos };
}

export function deleteRangeAtomic(text: string, start: number, end: number): { text: string; pos: number } {
  const r = expandToAtomic(text, start, end);
  return { text: text.slice(0, r.start) + text.slice(r.end), pos: r.start };
}

export function insertAtomic(text: string, pos: number, insertion: string): { text: string; pos: number } {
  const at = snapCaret(text, pos, "after");
  return { text: text.slice(0, at) + insertion + text.slice(at), pos: at + insertion.length };
}

export function moveCaret(text: string, pos: number, dir: -1 | 1): number {
  if (dir < 0) {
    if (pos <= 0) return 0;
    const prev = pos - 1;
    const span = tokenSpans(text).find((s) => s.start <= prev && prev < s.end);
    if (span?.kind === "tag") return span.start;
    return previousCodePointBoundary(text, pos);
  }
  if (pos >= text.length) return text.length;
  const span = tokenSpans(text).find((s) => s.start <= pos && pos < s.end);
  if (span?.kind === "tag") return span.end;
  return nextCodePointBoundary(text, pos);
}

export function tagsIntact(text: string): boolean {
  return parseDocument(text).every((t) => t.kind !== "tag" || /^<\/?[A-Za-z][\w:-]*\d*\/?>$/.test(t.value));
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

export function marksFromPrefs(marks: MarkPrefs | undefined): ViewMarks {
  const m = { ...DEFAULT_MARKS };
  if (!marks) return m;
  m.whitespace = marks.whitespace;
  m.nbsp = marks.nbsp;
  m.bidi = marks.bidi;
  m.glossary = marks.glossary;
  m.translated = marks.translated;
  m.untranslated = marks.untranslated;
  m.noted = marks.noted;
  m.nonUnique = marks.non_unique;
  m.autoPopulated = marks.auto_populated;
  m.alternative = marks.alternative;
  m.paragraphStart = marks.paragraph_start;
  m.displaySource = marks.display_source;
  m.languageChecker = marks.language_checker;
  m.fontFallback = marks.font_fallback;
  if (marks.modification === "selected" || marks.modification === "all" || marks.modification === "none") {
    m.modification = marks.modification;
  }
  return m;
}

export function prefsFromMarks(marks: ViewMarks): MarkPrefs {
  return {
    whitespace: marks.whitespace,
    nbsp: marks.nbsp,
    bidi: marks.bidi,
    glossary: marks.glossary,
    translated: marks.translated,
    untranslated: marks.untranslated,
    noted: marks.noted,
    non_unique: marks.nonUnique,
    auto_populated: marks.autoPopulated,
    alternative: marks.alternative,
    paragraph_start: marks.paragraphStart,
    display_source: marks.displaySource,
    language_checker: marks.languageChecker,
    font_fallback: marks.fontFallback,
    modification: marks.modification,
  };
}

export type MarkSpan = {
  /** Rendered text; visible marker glyphs may be longer than the source. */
  text: string;
  cls: string[];
  /** Number of UTF-16 units consumed from the undecorated model text. */
  sourceLength: number;
};

export function decorateText(text: string, marks: ViewMarks, glossary: string[] = []): MarkSpan[] {
  const spans: MarkSpan[] = [];
  const terms = glossary.filter(Boolean).sort((a, b) => b.length - a.length);
  let i = 0;
  while (i < text.length) {
    const ch = text[i]!;
    if (marks.nbsp && ch === "\u00a0") {
      spans.push({ text: "⍽", cls: ["mark-nbsp"], sourceLength: 1 });
      i += 1;
      continue;
    }
    if (marks.bidi && "\u200e\u200f\u202a\u202b\u202c".includes(ch)) {
      spans.push({
        text: ch === "\u200e" ? "LRM" : ch === "\u200f" ? "RLM" : "BIDI",
        cls: ["mark-bidi"],
        sourceLength: 1,
      });
      i += 1;
      continue;
    }
    if (marks.whitespace && (ch === " " || ch === "\t" || ch === "\n")) {
      spans.push({
        text: ch === " " ? "·" : ch === "\t" ? "→" : "¶\n",
        cls: ["mark-ws"],
        sourceLength: 1,
      });
      i += 1;
      continue;
    }
    if (marks.glossary && terms.length) {
      const rest = text.slice(i);
      const hit = terms.find((t) => rest.toLowerCase().startsWith(t.toLowerCase()));
      if (hit) {
        spans.push({
          text: text.slice(i, i + hit.length),
          cls: ["mark-glossary"],
          sourceLength: hit.length,
        });
        i += hit.length;
        continue;
      }
    }
    spans.push({ text: ch, cls: [], sourceLength: 1 });
    i += 1;
  }
  return mergeSpans(spans);
}

function mergeSpans(spans: MarkSpan[]): MarkSpan[] {
  const out: MarkSpan[] = [];
  for (const s of spans) {
    const last = out[out.length - 1];
    if (last && last.cls.join() === s.cls.join()) {
      last.text += s.text;
      last.sourceLength += s.sourceLength;
    }
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

export function switchCase(text: string, mode: ChangeCaseMode): string {
  return changeCase(text, mode);
}
