/** Java `org.omegat.gui.editor.Document3` — active translation range, dirty flag, tag atoms. */

export type StyledSpan = { start: number; end: number; style: string };

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
    if (offset < 0 || offset > this.text.length) {
      throw new Error("BadLocationException");
    }
    this.text = this.text.slice(0, offset) + inserted + this.text.slice(offset);
  }
  remove(offset: number, length: number): void {
    this.text = this.text.slice(0, offset) + this.text.slice(offset + length);
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
  const full = new StyledDocument();
  full.text = doc.fullText;
  full.insertString(offset, text);
  const delta = text.length;
  let translationStart = doc.translationStart;
  let translationEnd = doc.translationEnd;
  if (offset <= translationStart) translationStart += delta;
  if (offset < translationEnd) translationEnd += delta;
  return { ...doc, fullText: full.text, translationStart, translationEnd, dirty: true };
}

export function replaceEditText(doc: Document3State, text: string): Document3State {
  return { ...doc, translation: text, activeStart: 0, activeEnd: text.length, dirty: true };
}

export function insertText(doc: Document3State, text: string, at?: number): Document3State {
  const pos = at ?? doc.activeEnd;
  if (doc.tagsAtomic && isInsideTag(doc.translation, pos)) return doc;
  const translation = doc.translation.slice(0, pos) + text + doc.translation.slice(pos);
  return { ...doc, translation, activeStart: pos, activeEnd: pos + text.length, dirty: true };
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
  return { ...doc, dirty: false, activeStart: 0, activeEnd: doc.translation.length };
}

export function markSpan(doc: Document3State, start: number, end: number, style: string): Document3State {
  return { ...doc, spans: [...doc.spans, { start, end, style }] };
}
