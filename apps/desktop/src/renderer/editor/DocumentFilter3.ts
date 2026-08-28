/** Java `org.omegat.gui.editor.DocumentFilter3` — edits stay inside the translation range. */
import { FilterBypass } from "./FilterBypass";

export type FilterDocument = {
  text: string;
  editMode: boolean;
  trustedChangesInProgress: boolean;
  translationStart: number;
  translationEnd: number;
  textBeingComposed: boolean;
  allowTagEditing: boolean;
};

export type FilterAttrs = { composed?: boolean } | null;

export type FilterResult = { applied: boolean; doc: FilterDocument; bypass: FilterBypass };

export function createFilterDocument(text: string, translationStart: number, translationEnd: number): FilterDocument {
  return {
    text,
    editMode: true,
    trustedChangesInProgress: false,
    translationStart,
    translationEnd,
    textBeingComposed: false,
    allowTagEditing: true,
  };
}

export function isOffsetOutsideTranslationBounds(doc: FilterDocument, offset: number, length: number): boolean {
  return offset < doc.translationStart || offset + length > doc.translationEnd;
}

export function isPossible(doc: FilterDocument, offset: number, length: number): boolean {
  if (doc.trustedChangesInProgress) return true;
  if (!doc.editMode || isOffsetOutsideTranslationBounds(doc, offset, length)) return false;
  if (!doc.allowTagEditing && isInsideTag(doc.text, offset)) return false;
  return true;
}

const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;

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

/**
 * Java `replace(FilterBypass, offset, length, text, attrs)`:
 * set composed from attrs, then `isPossible` then `fb.replace`.
 */
export function replace(
  doc: FilterDocument,
  offset: number,
  length: number,
  text: string,
  attrs: FilterAttrs = null,
  bypass?: FilterBypass,
): FilterResult {
  let next = doc;
  if (attrs?.composed) next = { ...next, textBeingComposed: true };
  const fb = bypass ?? new FilterBypass(next);
  fb.doc = { ...fb.doc, textBeingComposed: next.textBeingComposed };
  if (!isPossible(fb.doc, offset, length)) return { applied: false, doc: fb.doc, bypass: fb };
  fb.replace(offset, length, text);
  return { applied: true, doc: fb.doc, bypass: fb };
}

export class DocumentFilter3 {
  replace(
    doc: FilterDocument,
    offset: number,
    length: number,
    text: string,
    attrs: FilterAttrs = null,
    bypass?: FilterBypass,
  ): FilterResult {
    return replace(doc, offset, length, text, attrs, bypass);
  }
}
