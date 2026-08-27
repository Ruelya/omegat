#!/usr/bin/env python3
"""Write real TS implementations for each Java gui/editor class (one file = one class).

Does not overwrite Document3.ts or IEditor.ts.
"""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ED = ROOT / "apps/desktop/src/renderer/editor"

MARK = '''/** Java `org.omegat.gui.editor.mark.Mark`. */
export type Mark = {
  startOffset: number;
  endOffset: number;
  painter: string;
  toolTipText?: string;
  entryPartSource: boolean;
};

export function mark(start: number, end: number, painter: string, toolTipText?: string, source = false): Mark {
  return { startOffset: start, endOffset: end, painter, toolTipText, entryPartSource: source };
}
'''

IMARKER = '''/** Java `org.omegat.gui.editor.mark.IMarker`. */
import type { Mark } from "./Mark";

export type MarkerInput = {
  sourceText: string;
  translationText: string;
  isActive: boolean;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  enabled?: boolean;
};

export interface IMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null;
}
'''

ABSTRACT = '''/** Java `org.omegat.gui.editor.mark.AbstractMarker`. */
import type { IMarker, MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export abstract class AbstractMarker implements IMarker {
  enabled = true;
  abstract getMarksForEntry(input: MarkerInput): Mark[] | null;
  isEnabled(): boolean {
    return this.enabled;
  }
}
'''

def write(rel: str, text: str) -> None:
    p = ED / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def main() -> None:
    write("mark/Mark.ts", MARK)
    write("mark/IMarker.ts", IMARKER)
    write("mark/AbstractMarker.ts", ABSTRACT)

    write(
        "mark/WhitespaceMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.WhitespaceMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class WhitespaceMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.sourceText) return null;
    const text = input.isActive || !input.translationText ? input.sourceText : input.translationText;
    const source = input.isActive || !input.translationText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      const ch = text[i]!;
      if (ch === " ") out.push(mark(i, i + 1, "·", "SPACE", source));
      else if (ch === "\\t") out.push(mark(i, i + 1, "»", "TAB", source));
      else if (ch === "\\n") out.push(mark(i, i + 1, "¶", "LF", source));
    }
    return out;
  }
}

export function whitespaceMarker(text: string) {
  return new WhitespaceMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/NBSPMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.NBSPMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class NBSPMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.isActive ? input.translationText || input.sourceText : input.sourceText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      if (text[i] === "\\u00a0") out.push(mark(i, i + 1, "nbsp", "NBSP"));
    }
    return out;
  }
}

export function nbspMarker(text: string) {
  return new NBSPMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/BidiMarkers.ts",
        '''/** Java `org.omegat.gui.editor.mark.BidiMarkers`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const BIDI = "\\u200e\\u200f\\u202a\\u202b\\u202c\\u202d\\u202e\\u2066\\u2067\\u2068\\u2069";

export class BidiMarkers extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText || input.sourceText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      if (BIDI.includes(text[i]!)) out.push(mark(i, i + 1, "bidi", "BIDI"));
    }
    return out;
  }
}

export function bidiMarkers(text: string) {
  return new BidiMarkers().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/ProtectedPartsMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.ProtectedPartsMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const TAG = /<\\/?[A-Za-z][\\w:-]*\\d*\\/?>/g;

export class ProtectedPartsMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText || input.sourceText;
    const out: Mark[] = [];
    for (const m of text.matchAll(TAG)) {
      out.push(mark(m.index ?? 0, (m.index ?? 0) + m[0].length, "protected", m[0]));
    }
    return out;
  }
}

export function protectedPartsMarker(text: string) {
  return new ProtectedPartsMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/AltTranslationsMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.AltTranslationsMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class AltTranslationsMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.isAlt || !input.translationText) return null;
    return [mark(0, input.translationText.length, "alt", "ALT")];
  }
}

export function altTranslationsMarker(isAlt: boolean, len: number) {
  return isAlt && len > 0 ? [mark(0, len, "alt", "ALT")] : [];
}
''',
    )

    write(
        "mark/ComesFromAutoTMMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.ComesFromAutoTMMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromAutoTMMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.fromAuto || !input.translationText) return null;
    return [mark(0, input.translationText.length, "auto-tm", "AUTO_TM")];
  }
}

export function comesFromAutoTMMarker(fromAuto: boolean, len: number) {
  return fromAuto && len > 0 ? [mark(0, len, "auto-tm", "AUTO_TM")] : [];
}
''',
    )

    write(
        "mark/ComesFromMTMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.ComesFromMTMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromMTMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.fromMt || !input.translationText) return null;
    return [mark(0, input.translationText.length, "mt", "MT")];
  }
}

export function comesFromMTMarker(fromMt: boolean, len: number) {
  return fromMt && len > 0 ? [mark(0, len, "mt", "MT")] : [];
}
''',
    )

    write(
        "mark/FontFallbackMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.FontFallbackMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class FontFallbackMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText || input.sourceText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      const u = text.charCodeAt(i);
      if (u > 0x024f && u < 0x1f000) out.push(mark(i, i + 1, "font-fallback"));
    }
    return out;
  }
}

export function fontFallbackMarker(text: string) {
  return new FontFallbackMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/RemoveTagMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.RemoveTagMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const TAG = /<\\/?[A-Za-z][\\w:-]*\\d*\\/?>/g;

export class RemoveTagMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const src = new Set(input.sourceText.match(TAG) ?? []);
    const out: Mark[] = [];
    for (const m of input.translationText.matchAll(TAG)) {
      if (!src.has(m[0])) out.push(mark(m.index ?? 0, (m.index ?? 0) + m[0].length, "remove-tag", m[0]));
    }
    return out;
  }
}

export function removeTagMarker(source: string, target: string) {
  return new RemoveTagMarker().getMarksForEntry({ sourceText: source, translationText: target, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/ReplaceMarker.ts",
        '''/** Java `org.omegat.gui.editor.mark.ReplaceMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ReplaceMarker extends AbstractMarker {
  needle = "";
  replacement = "";
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !this.needle) return null;
    const text = input.translationText || input.sourceText;
    const out: Mark[] = [];
    let i = 0;
    while (i < text.length) {
      const at = text.indexOf(this.needle, i);
      if (at < 0) break;
      out.push(mark(at, at + this.needle.length, "replace", this.replacement));
      i = at + this.needle.length;
    }
    return out;
  }
}

export function replaceMarker(text: string, needle: string, replacement: string) {
  const m = new ReplaceMarker();
  m.needle = needle;
  m.replacement = replacement;
  return m.getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
''',
    )

    write(
        "mark/EntryMarks.ts",
        '''/** Java `org.omegat.gui.editor.mark.EntryMarks`. */
import type { Mark } from "./Mark";

export type EntryMarks = {
  sourceMarks: Mark[];
  translationMarks: Mark[];
};

export function emptyEntryMarks(): EntryMarks {
  return { sourceMarks: [], translationMarks: [] };
}
''',
    )

    write(
        "mark/CalcMarkersThread.ts",
        '''/** Java `org.omegat.gui.editor.mark.CalcMarkersThread`. */
import type { IMarker, MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export function calcMarkers(markers: IMarker[], input: MarkerInput): Mark[] {
  const out: Mark[] = [];
  for (const m of markers) {
    const marks = m.getMarksForEntry(input);
    if (marks) out.push(...marks);
  }
  return out;
}
''',
    )

    write(
        "mark/BidiPainter.ts",
        '''/** Java `org.omegat.gui.editor.mark.BidiPainter`. */
export function paintBidi(ch: string): string {
  return `bidi:${ch.codePointAt(0)?.toString(16) ?? "?"}`;
}
''',
    )

    write(
        "mark/SymbolPainter.ts",
        '''/** Java `org.omegat.gui.editor.mark.SymbolPainter`. */
export class SymbolPainter {
  constructor(public color: string, public symbol: string) {}
  paint(): string {
    return this.symbol;
  }
}
''',
    )

    write(
        "mark/TransparentHighlightPainter.ts",
        '''/** Java `org.omegat.gui.editor.mark.TransparentHighlightPainter`. */
export class TransparentHighlightPainter {
  constructor(public color: string, public alpha = 0.35) {}
  css(): string {
    return `color-mix(in srgb, ${this.color} ${Math.round(this.alpha * 100)}%, transparent)`;
  }
}
''',
    )

    write(
        "mark/markers.ts",
        '''/** Compose Java `gui.editor.mark.*` intervals. */
import { altTranslationsMarker } from "./AltTranslationsMarker";
import { bidiMarkers } from "./BidiMarkers";
import { comesFromAutoTMMarker } from "./ComesFromAutoTMMarker";
import { comesFromMTMarker } from "./ComesFromMTMarker";
import { fontFallbackMarker } from "./FontFallbackMarker";
import { nbspMarker } from "./NBSPMarker";
import { protectedPartsMarker } from "./ProtectedPartsMarker";
import { removeTagMarker } from "./RemoveTagMarker";
import { replaceMarker } from "./ReplaceMarker";
import { whitespaceMarker } from "./WhitespaceMarker";
import type { Mark } from "./Mark";

export type MarkInterval = { start: number; end: number; kind: string; text: string };

function asInterval(m: Mark, kind: string): MarkInterval {
  return { start: m.startOffset, end: m.endOffset, kind, text: m.toolTipText ?? m.painter };
}

export function allMarkers(input: {
  text: string;
  source?: string;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
}): MarkInterval[] {
  const t = input.text;
  return [
    ...whitespaceMarker(t).map((m) => asInterval(m, "whitespace")),
    ...nbspMarker(t).map((m) => asInterval(m, "nbsp")),
    ...bidiMarkers(t).map((m) => asInterval(m, "bidi")),
    ...protectedPartsMarker(t).map((m) => asInterval(m, "protected")),
    ...altTranslationsMarker(!!input.isAlt, t.length).map((m) => asInterval(m, "alt")),
    ...comesFromAutoTMMarker(!!input.fromAuto, t.length).map((m) => asInterval(m, "auto-tm")),
    ...comesFromMTMarker(!!input.fromMt, t.length).map((m) => asInterval(m, "mt")),
    ...fontFallbackMarker(t).map((m) => asInterval(m, "font-fallback")),
    ...removeTagMarker(input.source ?? "", t).map((m) => asInterval(m, "remove-tag")),
  ];
}

export {
  whitespaceMarker,
  nbspMarker,
  bidiMarkers,
  protectedPartsMarker,
  altTranslationsMarker,
  comesFromAutoTMMarker,
  comesFromMTMarker,
  fontFallbackMarker,
  removeTagMarker,
  replaceMarker,
};
''',
    )

    # Autocompleter
    write(
        "autocompleter/AutoCompleterItem.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterItem`. */
export type AutoCompleterItem = {
  payload: string;
  extras: string[];
  cursorAdjust: number;
  kind: string;
};

export function item(payload: string, kind: string, extras: string[] = [], cursorAdjust = 0): AutoCompleterItem {
  return { payload, extras, cursorAdjust, kind };
}
''',
    )

    write(
        "autocompleter/IAutoCompleter.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.IAutoCompleter`. */
import type { AutoCompleterItem } from "./AutoCompleterItem";

export interface IAutoCompleter {
  updatePopup(): AutoCompleterItem[];
  confirm(): string | null;
  isVisible(): boolean;
}
''',
    )

    write(
        "autocompleter/AbstractAutoCompleterView.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AbstractAutoCompleterView`. */
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AbstractAutoCompleterView {
  constructor(public name: string) {}
  abstract computeListData(wordChunk: string, onlyCurrentWord: boolean): AutoCompleterItem[];
  shouldPopUp(wordChunk: string): boolean {
    return wordChunk.length > 0;
  }
}
''',
    )

    write(
        "autocompleter/AutoCompleterListView.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterListView`. */
import { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AutoCompleterListView extends AbstractAutoCompleterView {
  selected = 0;
  items: AutoCompleterItem[] = [];
  select(i: number) {
    this.selected = Math.max(0, Math.min(i, this.items.length - 1));
  }
}
''',
    )

    write(
        "autocompleter/AutoCompleterTableView.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterTableView`. */
import { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AutoCompleterTableView extends AbstractAutoCompleterView {
  row = 0;
  col = 0;
  cells: AutoCompleterItem[][] = [];
}
''',
    )

    write(
        "autocompleter/AutoCompleterKeys.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterKeys`. */
export const AutoCompleterKeys = {
  confirm: "Enter",
  nextView: "ArrowRight",
  prevView: "ArrowLeft",
  nextItem: "ArrowDown",
  prevItem: "ArrowUp",
  close: "Escape",
} as const;
''',
    )

    write(
        "autocompleter/GlossaryAutoCompleterView.ts",
        '''/** Java glossary autocompleter view. */
import { AutoCompleterListView } from "./AutoCompleterListView";
import { item, type AutoCompleterItem } from "./AutoCompleterItem";

export class GlossaryAutoCompleterView extends AutoCompleterListView {
  terms: { source: string; target: string }[] = [];
  constructor() {
    super("glossary");
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return this.terms
      .filter((t) => t.source.toLowerCase().includes(p) || t.target.toLowerCase().includes(p))
      .map((t) => item(t.target, "glossary", [t.source]));
  }
}
''',
    )

    write(
        "autocompleter/AutoCompleter.ts",
        '''/** Java `org.omegat.gui.editor.autocompleter.AutoCompleter`. */
import type { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";
import { AutoCompleterKeys } from "./AutoCompleterKeys";
import { GlossaryAutoCompleterView } from "./GlossaryAutoCompleterView";
import type { IAutoCompleter } from "./IAutoCompleter";

export class AutoCompleter implements IAutoCompleter {
  views: AbstractAutoCompleterView[] = [new GlossaryAutoCompleterView()];
  viewIndex = 0;
  visible = false;
  items: AutoCompleterItem[] = [];
  selected = 0;

  setViews(views: AbstractAutoCompleterView[]) {
    this.views = views;
    this.viewIndex = 0;
  }

  updatePopup(wordChunk = ""): AutoCompleterItem[] {
    const view = this.views[this.viewIndex];
    this.items = view ? view.computeListData(wordChunk, true) : [];
    this.visible = this.items.length > 0;
    this.selected = 0;
    return this.items;
  }

  confirm(): string | null {
    const it = this.items[this.selected];
    this.visible = false;
    return it?.payload ?? null;
  }

  isVisible(): boolean {
    return this.visible;
  }

  nextView() {
    if (!this.views.length) return;
    this.viewIndex = (this.viewIndex + 1) % this.views.length;
  }

  keys() {
    return AutoCompleterKeys;
  }
}
''',
    )

    write(
        "autotext/Autotext.ts",
        '''/** Java `org.omegat.gui.editor.autotext.Autotext`. */
export type AutotextEntry = { shortcut: string; full: string; comment: string };

export function parseAutotext(raw: string): AutotextEntry[] {
  return raw
    .split(/\\n+/)
    .map((ln) => ln.trim())
    .filter(Boolean)
    .map((ln) => {
      const [shortcut = "", full = "", comment = ""] = ln.split("=");
      return { shortcut, full, comment };
    });
}

export function matchAutotext(entries: AutotextEntry[], chunk: string): AutotextEntry[] {
  const p = chunk.toLowerCase();
  return entries.filter((e) => e.shortcut.toLowerCase().startsWith(p) || e.full.toLowerCase().includes(p));
}
''',
    )

    write(
        "autotext/AutotextTableModel.ts",
        '''/** Java `org.omegat.gui.editor.autotext.AutotextTableModel`. */
import type { AutotextEntry } from "./Autotext";

export class AutotextTableModel {
  constructor(public rows: AutotextEntry[] = []) {}
  getRowCount() {
    return this.rows.length;
  }
  getValueAt(row: number, col: number): string {
    const r = this.rows[row];
    if (!r) return "";
    return col === 0 ? r.shortcut : col === 1 ? r.full : r.comment;
  }
}
''',
    )

    write(
        "autotext/AutotextAutoCompleterView.ts",
        '''/** Java `org.omegat.gui.editor.autotext.AutotextAutoCompleterView`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { matchAutotext, parseAutotext, type AutotextEntry } from "./Autotext";

export class AutotextAutoCompleterView extends AutoCompleterListView {
  entries: AutotextEntry[] = [];
  constructor(raw = "") {
    super("autotext");
    this.entries = parseAutotext(raw);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    return matchAutotext(this.entries, wordChunk).map((e) => item(e.full, "autotext", [e.shortcut, e.comment]));
  }
}
''',
    )

    write(
        "chartable/CharTableModel.ts",
        '''/** Java `org.omegat.gui.editor.chartable.CharTableModel`. */
export class CharTableModel {
  constructor(public chars: string) {}
  cell(i: number): string {
    return this.chars[i] ?? "";
  }
  size() {
    return this.chars.length;
  }
}
''',
    )

    write(
        "chartable/CharTableRenderer.ts",
        '''/** Java `org.omegat.gui.editor.chartable.CharTableRenderer`. */
export function renderChar(ch: string): string {
  if (!ch) return "";
  return `${ch} U+${ch.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`;
}
''',
    )

    write(
        "chartable/CharTableAutoCompleterView.ts",
        '''/** Java `org.omegat.gui.editor.chartable.CharTableAutoCompleterView`. */
import { AutoCompleterTableView } from "../autocompleter/AutoCompleterTableView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { CharTableModel } from "./CharTableModel";

export class CharTableAutoCompleterView extends AutoCompleterTableView {
  model: CharTableModel;
  constructor(chars = "") {
    super("chartable");
    this.model = new CharTableModel(chars);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return [...this.model.chars]
      .filter((c) => !p || c.toLowerCase().includes(p))
      .map((c) => item(c, "chartable"));
  }
}
''',
    )

    write(
        "history/WordCompleter.ts",
        '''/** Java `org.omegat.gui.editor.history.WordCompleter`. */
export function completeWords(translations: string[], prefix: string): string[] {
  if (!prefix) return [];
  const p = prefix.toLowerCase();
  const seen = new Set<string>();
  const out: string[] = [];
  for (const text of translations) {
    for (const w of text.split(/[^\\p{L}\\p{N}']+/u)) {
      if (w.length > 1 && w.toLowerCase().startsWith(p) && w.toLowerCase() !== p && !seen.has(w)) {
        seen.add(w);
        out.push(w);
      }
    }
  }
  return out;
}
''',
    )

    write(
        "history/WordPredictor.ts",
        '''/** Java `org.omegat.gui.editor.history.WordPredictor` — next-word model. */
export type PredictorModel = Map<string, Map<string, number>>;

export function trainPredictor(translations: string[]): PredictorModel {
  const model: PredictorModel = new Map();
  for (const text of translations) {
    const words = text.split(/[^\\p{L}\\p{N}']+/u).filter(Boolean);
    for (let i = 0; i < words.length - 1; i++) {
      const a = words[i]!.toLowerCase();
      const b = words[i + 1]!;
      if (!model.has(a)) model.set(a, new Map());
      const inner = model.get(a)!;
      inner.set(b, (inner.get(b) ?? 0) + 1);
    }
  }
  return model;
}

export function predictNext(model: PredictorModel, prev: string): { word: string; pct: number }[] {
  const trailing = /\\s$/.test(prev);
  const tokens = prev.trim().split(/\\s+/).filter(Boolean);
  const seed = trailing ? tokens.at(-1) ?? "" : tokens.at(-2) ?? "";
  const ctx = trailing ? "" : tokens.at(-1) ?? "";
  if (!seed) return [];
  const nexts = model.get(seed.toLowerCase());
  if (!nexts) return [];
  const total = [...nexts.values()].reduce((a, b) => a + b, 0);
  return [...nexts.entries()]
    .filter(([w]) => !ctx || (w.startsWith(ctx) && w !== ctx))
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([word, n]) => ({ word, pct: Math.round((n / total) * 100) }));
}
''',
    )

    write(
        "history/HistoryCompleter.ts",
        '''/** Java `org.omegat.gui.editor.history.HistoryCompleter`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { completeWords } from "./WordCompleter";

export class HistoryCompleter extends AutoCompleterListView {
  translations: string[] = [];
  constructor() {
    super("history");
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    return completeWords(this.translations, wordChunk).map((w) => item(w, "history"));
  }
}
''',
    )

    write(
        "history/HistoryPredictor.ts",
        '''/** Java `org.omegat.gui.editor.history.HistoryPredictor`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { predictNext, trainPredictor, type PredictorModel } from "./WordPredictor";

export class HistoryPredictor extends AutoCompleterListView {
  model: PredictorModel = new Map();
  constructor() {
    super("history-predict");
  }
  train(translations: string[]) {
    this.model = trainPredictor(translations);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    return predictNext(this.model, wordChunk).map((p) => item(p.word, "history-predict", [`${p.pct}%`]));
  }
}
''',
    )

    write(
        "filter/SearchFilter.ts",
        '''/** Java `org.omegat.gui.editor.filter.SearchFilter`. */
export type SearchFilter = { query: string; regex: boolean };

export function searchFilterMatches(source: string, translation: string, f: SearchFilter): boolean {
  if (!f.query) return true;
  const hay = `${source}\\n${translation}`;
  if (f.regex) {
    try {
      return new RegExp(f.query, "i").test(hay);
    } catch {
      return hay.toLowerCase().includes(f.query.toLowerCase());
    }
  }
  return hay.toLowerCase().includes(f.query.toLowerCase());
}
''',
    )

    write(
        "filter/ReplaceFilter.ts",
        '''/** Java `org.omegat.gui.editor.filter.ReplaceFilter`. */
export type ReplaceFilter = { needle: string; replacement: string };

export function applyReplace(text: string, f: ReplaceFilter): string {
  if (!f.needle) return text;
  return text.split(f.needle).join(f.replacement);
}
''',
    )

    write(
        "filter/FilterBarSearch.ts",
        '''/** Java `org.omegat.gui.editor.filter.FilterBarSearch`. */
import { searchFilterMatches, type SearchFilter } from "./SearchFilter";

export function filterBarSearch(entries: { source: string; translation: string }[], f: SearchFilter) {
  return entries.filter((e) => searchFilterMatches(e.source, e.translation, f));
}
''',
    )

    write(
        "filter/FilterBarReplace.ts",
        '''/** Java `org.omegat.gui.editor.filter.FilterBarReplace`. */
import { applyReplace, type ReplaceFilter } from "./ReplaceFilter";

export function filterBarReplace(text: string, f: ReplaceFilter): string {
  return applyReplace(text, f);
}
''',
    )

    # Core editor classes
    write(
        "EditorController.ts",
        '''/** Java `org.omegat.gui.editor.EditorController` — IEditor implementation host. */
import { IEditor } from "./IEditor";
import { MarkerController } from "./MarkerController";
import { TranslationUndoManager } from "./TranslationUndoManager";

export class EditorController {
  readonly editor = IEditor;
  readonly markers = new MarkerController();
  readonly undo = new TranslationUndoManager();

  getCurrentTranslation() {
    return this.editor.getCurrentTranslation();
  }
  replaceEditText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.replaceEditText(text);
  }
  insertText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.insertText(text);
  }
  commitAndDeactivate() {
    return this.editor.commitAndDeactivate();
  }
}
''',
    )

    write(
        "EditorTextArea3.ts",
        '''/** Java `org.omegat.gui.editor.EditorTextArea3`. */
import { createDocument3, type Document3State } from "./Document3";
import { deleteBackwardAtomic } from "../lib/editor-doc";

export class EditorTextArea3 {
  doc: Document3State;
  constructor(source = "", translation = "") {
    this.doc = createDocument3(source, translation);
  }
  getText() {
    return this.doc.translation;
  }
  setText(text: string) {
    this.doc = { ...this.doc, translation: text, activeEnd: text.length, dirty: true };
  }
  deleteBackward() {
    this.doc = { ...this.doc, translation: deleteBackwardAtomic(this.doc.translation), dirty: true };
  }
}
''',
    )

    write(
        "DocumentFilter3.ts",
        '''/** Java `org.omegat.gui.editor.DocumentFilter3` — tag atoms are not split. */
const TAG = /<\\/?[A-Za-z][\\w:-]*\\d*\\/?>/g;

export function isInsideTag(text: string, offset: number): boolean {
  for (const m of text.matchAll(TAG)) {
    const s = m.index ?? 0;
    if (offset > s && offset < s + m[0].length) return true;
  }
  return false;
}

export function allowInsert(text: string, offset: number): boolean {
  return !isInsideTag(text, offset);
}
''',
    )

    write(
        "EditorSettings.ts",
        '''/** Java `org.omegat.gui.editor.EditorSettings`. */
export type EditorSettingsState = {
  markWhitespace: boolean;
  markNbsp: boolean;
  markBidi: boolean;
  displaySegmentSources: boolean;
  markTranslated: boolean;
  markUntranslated: boolean;
};

export function defaultEditorSettings(): EditorSettingsState {
  return {
    markWhitespace: true,
    markNbsp: true,
    markBidi: true,
    displaySegmentSources: true,
    markTranslated: true,
    markUntranslated: true,
  };
}
''',
    )

    write(
        "IEditorSettings.ts",
        '''/** Java `org.omegat.gui.editor.IEditorSettings`. */
import type { EditorSettingsState } from "./EditorSettings";

export type IEditorSettings = EditorSettingsState;
''',
    )

    write(
        "IEditorFilter.ts",
        '''/** Java `org.omegat.gui.editor.IEditorFilter`. */
export type IEditorFilter = {
  kind: "untranslated" | "unique" | "noted" | "search" | "none";
  query?: string;
  allowed(entry: { translation: string; note?: string; unique?: boolean }): boolean;
};

export function makeFilter(kind: IEditorFilter["kind"], query?: string): IEditorFilter {
  return {
    kind,
    query,
    allowed(entry) {
      if (kind === "untranslated") return !entry.translation;
      if (kind === "noted") return !!entry.note;
      if (kind === "unique") return entry.unique !== false;
      if (kind === "search") return `${entry.translation}`.includes(query ?? "");
      return true;
    },
  };
}
''',
    )

    write(
        "EditorUtils.ts",
        '''/** Java `org.omegat.gui.editor.EditorUtils`. */
export function removeDirectionChars(s: string): string {
  return s.replace(/[\\u200e\\u200f\\u202a-\\u202e\\u2066-\\u2069]/g, "");
}

export function changeCase(s: string, mode: "upper" | "lower" | "title" | "sentence"): string {
  if (mode === "upper") return s.toUpperCase();
  if (mode === "lower") return s.toLowerCase();
  if (mode === "title") return s.replace(/\\S+/g, (w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
  return s.charAt(0).toUpperCase() + s.slice(1);
}
''',
    )

    write(
        "EditorPopups.ts",
        '''/** Java `org.omegat.gui.editor.EditorPopups`. */
export type PopupItem = { id: string; label: string };

export function editorPopups(): PopupItem[] {
  return [
    { id: "edit.insert-source", label: "Insert source" },
    { id: "edit.insert-translation", label: "Insert match" },
    { id: "edit.glossary", label: "Add glossary" },
    { id: "edit.register-untranslated", label: "Untranslated" },
  ];
}
''',
    )

    write(
        "IPopupMenuConstructor.ts",
        '''/** Java `org.omegat.gui.editor.IPopupMenuConstructor`. */
export type IPopupMenuConstructor = (x: number, y: number) => void;
''',
    )

    write(
        "SegmentBuilder.ts",
        '''/** Java `org.omegat.gui.editor.SegmentBuilder`. */
export type BuiltSegment = {
  source: string;
  translation: string;
  active: boolean;
  number: number;
};

export function buildSegment(number: number, source: string, translation: string, active: boolean): BuiltSegment {
  return { number, source, translation, active };
}
''',
    )

    write(
        "SegmentHistory.ts",
        '''/** Java `org.omegat.gui.editor.SegmentHistory`. */
export class SegmentHistory {
  back: number[] = [];
  forward: number[] = [];
  visit(n: number) {
    this.back.push(n);
    this.forward = [];
  }
  goBack(): number | undefined {
    const n = this.back.pop();
    if (n !== undefined) this.forward.push(n);
    return this.back.at(-1);
  }
  goForward(): number | undefined {
    const n = this.forward.pop();
    if (n !== undefined) this.back.push(n);
    return n;
  }
}
''',
    )

    write(
        "TranslationUndoManager.ts",
        '''/** Java `org.omegat.gui.editor.TranslationUndoManager`. */
export class TranslationUndoManager {
  undoStack: string[] = [];
  redoStack: string[] = [];
  remember(text: string) {
    this.undoStack.push(text);
    this.redoStack = [];
  }
  undo(current: string): string {
    const prev = this.undoStack.pop();
    if (prev === undefined) return current;
    this.redoStack.push(current);
    return prev;
  }
  redo(current: string): string {
    const next = this.redoStack.pop();
    if (next === undefined) return current;
    this.undoStack.push(current);
    return next;
  }
}
''',
    )

    write(
        "MarkerController.ts",
        '''/** Java `org.omegat.gui.editor.MarkerController`. */
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { calcMarkers } from "./mark/CalcMarkersThread";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { ComesFromMTMarker } from "./mark/ComesFromMTMarker";
import { FontFallbackMarker } from "./mark/FontFallbackMarker";
import type { IMarker, MarkerInput } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { RemoveTagMarker } from "./mark/RemoveTagMarker";
import { ReplaceMarker } from "./mark/ReplaceMarker";
import { WhitespaceMarker } from "./mark/WhitespaceMarker";

export class MarkerController {
  markers: IMarker[] = [
    new WhitespaceMarker(),
    new NBSPMarker(),
    new BidiMarkers(),
    new ProtectedPartsMarker(),
    new AltTranslationsMarker(),
    new ComesFromAutoTMMarker(),
    new ComesFromMTMarker(),
    new FontFallbackMarker(),
    new RemoveTagMarker(),
    new ReplaceMarker(),
  ];

  process(input: MarkerInput): Mark[] {
    return calcMarkers(this.markers, input);
  }
}
''',
    )

    write(
        "ModificationInfoManager.ts",
        '''/** Java `org.omegat.gui.editor.ModificationInfoManager`. */
export type ModificationInfo = { author: string; date: string; origin?: string };

export function formatModification(info: ModificationInfo, withDate = true): string {
  return withDate ? `${info.author} ${info.date}` : info.author;
}
''',
    )

    write(
        "AlphabeticalMarkers.ts",
        '''/** Java `org.omegat.gui.editor.AlphabeticalMarkers`. */
export function alphabeticalMarker(index: number): string {
  let n = index;
  let s = "";
  do {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
  } while (n >= 0);
  return s;
}
''',
    )

    write(
        "CollapsibleBar.ts",
        '''/** Java `org.omegat.gui.editor.CollapsibleBar`. */
export type CollapsibleBarState = { collapsed: boolean; title: string };

export function toggleBar(bar: CollapsibleBarState): CollapsibleBarState {
  return { ...bar, collapsed: !bar.collapsed };
}
''',
    )

    write(
        "ViewParagraph.ts",
        '''/** Java `org.omegat.gui.editor.ViewParagraph`. */
export type ViewParagraph = { start: number; end: number; text: string };

export function paragraphs(text: string): ViewParagraph[] {
  const out: ViewParagraph[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i === text.length || text[i] === "\\n") {
      out.push({ start, end: i, text: text.slice(start, i) });
      start = i + 1;
    }
  }
  return out;
}
''',
    )

    write(
        "ViewLabel.ts",
        '''/** Java `org.omegat.gui.editor.ViewLabel`. */
export function viewLabel(n: number, source: boolean): string {
  return source ? `${n} ›` : `${n} <`;
}
''',
    )

    write(
        "UnderlineFactory.ts",
        '''/** Java `org.omegat.gui.editor.UnderlineFactory`. */
export type Underline = { style: "solid" | "wavy" | "dotted"; color: string };

export function underlineFor(kind: string): Underline {
  if (kind === "spell") return { style: "wavy", color: "#c00" };
  if (kind === "lt") return { style: "wavy", color: "#06c" };
  if (kind === "glossary") return { style: "dotted", color: "#080" };
  return { style: "solid", color: "#888" };
}
''',
    )

    write(
        "SegmentExportImport.ts",
        '''/** Java `org.omegat.gui.editor.SegmentExportImport`. */
export function exportSegment(source: string, translation: string): string {
  return `source\\t${source}\\ntarget\\t${translation}\\n`;
}

export function importSegment(raw: string): { source: string; translation: string } {
  const src = /source\\t(.*)/.exec(raw)?.[1] ?? "";
  const tgt = /target\\t(.*)/.exec(raw)?.[1] ?? "";
  return { source: src, translation: tgt };
}
''',
    )

    write(
        "TagAutoCompleterView.ts",
        '''/** Java `org.omegat.gui.editor.TagAutoCompleterView`. */
import { AutoCompleterListView } from "./autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "./autocompleter/AutoCompleterItem";

export class TagAutoCompleterView extends AutoCompleterListView {
  tags: string[] = [];
  constructor(tags: string[] = []) {
    super("tag");
    this.tags = tags;
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return this.tags.filter((t) => t.toLowerCase().includes(p)).map((t) => item(t, "tag"));
  }
}
''',
    )

    write(
        "index.ts",
        '''export * from "./Document3";
export * from "./IEditor";
export * from "./EditorController";
export * from "./mark/markers";
export * from "./autocompleter/AutoCompleter";
''',
    )

    print("wrote P7 class implementations")


if __name__ == "__main__":
    main()
