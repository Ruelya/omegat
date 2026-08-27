/** Java `org.omegat.gui.editor.Document3` — active translation range, dirty flag, tag atoms. */

export type StyledSpan = { start: number; end: number; style: string };

export type DocumentEditOptions = {
  trusted?: boolean;
  composed?: boolean;
};

export type Document3State = {
  source: string;
  translation: string;
  activeStart: number;
  activeEnd: number;
  dirty: boolean;
  tagsAtomic: boolean;
  spans: StyledSpan[];
  fullText: string;
  translationStart: number;
  translationEnd: number;
  editMode: boolean;
  trustedChangesInProgress: boolean;
  textBeingComposed: boolean;
};

/** Swing `DefaultStyledDocument` string buffer used by SegmentBuilder.insert. */
export class StyledDocument {
  text = "";
  insertString(offset: number, inserted: string): void {
    this.replace(offset, 0, inserted);
  }
  remove(offset: number, length: number): void {
    this.replace(offset, length, "");
  }
  replace(offset: number, length: number, inserted: string): void {
    assertLocation(this.text, offset, length);
    this.text = this.text.slice(0, offset) + inserted + this.text.slice(offset + length);
  }
  getLength(): number {
    return this.text.length;
  }
  getText(offset: number, length: number): string {
    return this.text.slice(offset, offset + length);
  }
}

export function createDocument3(source = "", translation = ""): Document3State {
  return {
    source,
    translation,
    activeStart: 0,
    activeEnd: translation.length,
    dirty: false,
    tagsAtomic: true,
    spans: [],
    fullText: translation,
    translationStart: 0,
    translationEnd: translation.length,
    editMode: true,
    trustedChangesInProgress: false,
    textBeingComposed: false,
  };
}

export function insertString(doc: Document3State, offset: number, text: string): Document3State {
  assertLocation(doc.fullText, offset, 0);
  const full = new StyledDocument();
  full.text = doc.fullText;
  full.insertString(offset, text);
  const delta = text.length;
  let translationStart = doc.translationStart;
  let translationEnd = doc.translationEnd;
  if (offset <= translationStart) {
    translationStart += delta;
    translationEnd += delta;
  } else if (offset < translationEnd) {
    translationEnd += delta;
  }
  return {
    ...doc,
    fullText: full.text,
    translationStart,
    translationEnd,
    translation: full.text.slice(translationStart, translationEnd),
    spans: shiftSpans(doc.spans, offset, 0, text.length),
  };
}

export function replaceEditText(doc: Document3State, text: string): Document3State {
  return applyDocumentEdit(doc, doc.translationStart, doc.translationEnd - doc.translationStart, text);
}

export function insertText(doc: Document3State, text: string, at?: number): Document3State {
  const pos = at ?? doc.activeEnd;
  if (pos < 0 || pos > doc.translation.length) throw new Error("BadLocationException");
  return applyDocumentEdit(doc, doc.translationStart + pos, 0, text);
}

const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;

function isInsideTag(text: string, offset: number): boolean {
  for (const m of text.matchAll(TAG)) {
    const s = m.index ?? 0;
    if (offset > s && offset < s + m[0].length) return true;
  }
  return false;
}

export function commitAndDeactivate(doc: Document3State): Document3State {
  return {
    ...doc,
    dirty: false,
    editMode: false,
    activeStart: 0,
    activeEnd: doc.translation.length,
    trustedChangesInProgress: false,
    textBeingComposed: false,
  };
}

export function markSpan(doc: Document3State, start: number, end: number, style: string): Document3State {
  assertLocation(doc.fullText, start, end - start);
  return { ...doc, spans: [...doc.spans, { start, end, style }] };
}

function assertLocation(text: string, offset: number, length: number): void {
  if (offset < 0 || length < 0 || offset + length > text.length) {
    throw new Error("BadLocationException");
  }
}

function shiftSpans(spans: StyledSpan[], offset: number, removed: number, inserted: number): StyledSpan[] {
  const removedEnd = offset + removed;
  const delta = inserted - removed;
  return spans.flatMap((span) => {
    if (span.end <= offset) return [span];
    if (span.start >= removedEnd) {
      return [{ ...span, start: span.start + delta, end: span.end + delta }];
    }
    // An edit through a painted range invalidates only the overlapping part;
    // the unaffected edges remain available to MarkerController.
    const nextStart = Math.min(span.start, offset);
    const nextEnd = Math.max(nextStart, span.end + delta);
    return nextEnd > nextStart ? [{ ...span, start: nextStart, end: nextEnd }] : [];
  });
}

/**
 * Apply a user or trusted document mutation through the active translation
 * positions. Off-range mutations are rejected unless trusted; tag interiors
 * remain atomic at both ends of a selection.
 */
export function applyDocumentEdit(
  doc: Document3State,
  offset: number,
  length: number,
  text: string,
  options: DocumentEditOptions = {},
): Document3State {
  assertLocation(doc.fullText, offset, length);
  const trusted = options.trusted ?? doc.trustedChangesInProgress;
  if (!trusted) {
    if (!doc.editMode || offset < doc.translationStart || offset + length > doc.translationEnd) return doc;
    const relativeStart = offset - doc.translationStart;
    const relativeEnd = relativeStart + length;
    if (
      doc.tagsAtomic &&
      (isInsideTag(doc.translation, relativeStart) || isInsideTag(doc.translation, relativeEnd))
    ) {
      return doc;
    }
  }

  const full = new StyledDocument();
  full.text = doc.fullText;
  full.replace(offset, length, text);
  const delta = text.length - length;
  let translationStart = doc.translationStart;
  let translationEnd = doc.translationEnd;
  if (trusted && offset + length <= translationStart) {
    translationStart += delta;
    translationEnd += delta;
  } else if (offset <= translationEnd) {
    translationEnd += delta;
  }
  const caret = Math.max(0, Math.min(translationEnd - translationStart, offset - translationStart + text.length));
  return {
    ...doc,
    fullText: full.text,
    translationStart,
    translationEnd,
    translation: full.text.slice(translationStart, translationEnd),
    activeStart: caret,
    activeEnd: caret,
    dirty: trusted ? doc.dirty : true,
    textBeingComposed: options.composed ?? doc.textBeingComposed,
    spans: shiftSpans(doc.spans, offset, length, text.length),
  };
}

export function extractTranslation(doc: Document3State): string | null {
  return doc.editMode ? doc.fullText.slice(doc.translationStart, doc.translationEnd) : null;
}

export function activateTranslation(doc: Document3State, start: number, end: number): Document3State {
  assertLocation(doc.fullText, start, end - start);
  return {
    ...doc,
    editMode: true,
    translationStart: start,
    translationEnd: end,
    translation: doc.fullText.slice(start, end),
    activeStart: 0,
    activeEnd: end - start,
  };
}

export function stopEditMode(doc: Document3State): Document3State {
  return { ...doc, editMode: false, trustedChangesInProgress: false, textBeingComposed: false };
}

export function setTrustedChangesInProgress(doc: Document3State, trusted: boolean): Document3State {
  return { ...doc, trustedChangesInProgress: trusted };
}

export function setTextBeingComposed(doc: Document3State, composed: boolean): Document3State {
  return { ...doc, textBeingComposed: composed };
}

export function setAlignment(
  doc: Document3State,
  beginOffset: number,
  endOffset: number,
  rightAligned: boolean,
): Document3State {
  return markSpan(doc, beginOffset, endOffset, rightAligned ? "align-right" : "align-left");
}

/** Mutable facade used by controller-style code while the store keeps POJOs. */
export class Document3 {
  constructor(public state: Document3State = createDocument3()) {}

  getTranslationStart(): number {
    return this.state.translationStart;
  }

  getTranslationEnd(): number {
    return this.state.translationEnd;
  }

  isEditMode(): boolean {
    return this.state.editMode;
  }

  extractTranslation(): string | null {
    return extractTranslation(this.state);
  }

  replace(offset: number, length: number, text: string, options?: DocumentEditOptions): boolean {
    const previous = this.state;
    this.state = applyDocumentEdit(this.state, offset, length, text, options);
    return this.state !== previous;
  }

  insertString(offset: number, text: string, options?: DocumentEditOptions): boolean {
    return this.replace(offset, 0, text, options);
  }

  remove(offset: number, length: number, options?: DocumentEditOptions): boolean {
    return this.replace(offset, length, "", options);
  }

  stopEditMode(): void {
    this.state = stopEditMode(this.state);
  }

  setTrustedChangesInProgress(trusted: boolean): void {
    this.state = setTrustedChangesInProgress(this.state, trusted);
  }

  setTextBeingComposed(composed: boolean): void {
    this.state = setTextBeingComposed(this.state, composed);
  }

  setAlignment(beginOffset: number, endOffset: number, rightAligned: boolean): void {
    this.state = setAlignment(this.state, beginOffset, endOffset, rightAligned);
  }
}
