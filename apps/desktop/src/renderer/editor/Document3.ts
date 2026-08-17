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
};

export function createDocument3(source = "", translation = ""): Document3State {
  return {
    source,
    translation,
    activeStart: 0,
    activeEnd: translation.length,
    dirty: false,
    tagsAtomic: true,
    spans: [],
  };
}

export function replaceEditText(doc: Document3State, text: string): Document3State {
  return { ...doc, translation: text, activeStart: 0, activeEnd: text.length, dirty: true };
}

export function insertText(doc: Document3State, text: string, at?: number): Document3State {
  const pos = at ?? doc.activeEnd;
  const translation = doc.translation.slice(0, pos) + text + doc.translation.slice(pos);
  return { ...doc, translation, activeStart: pos, activeEnd: pos + text.length, dirty: true };
}

export function commitAndDeactivate(doc: Document3State): Document3State {
  return { ...doc, dirty: false, activeStart: 0, activeEnd: doc.translation.length };
}

export function markSpan(doc: Document3State, start: number, end: number, style: string): Document3State {
  return { ...doc, spans: [...doc.spans, { start, end, style }] };
}
