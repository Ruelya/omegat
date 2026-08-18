/** Java `org.omegat.gui.editor.SegmentBuilder`. */
import { createDocument3, type Document3State } from "./Document3";

export type BuiltSegment = {
  source: string;
  translation: string;
  active: boolean;
  number: number;
  mark: string;
};

export const SEGMENT_MARKER_STRING = "<0000>";

export function createSegmentMarkText(segmentNumberInProject: number, paragraphStart = false): string {
  let text = SEGMENT_MARKER_STRING;
  const replacement = String(segmentNumberInProject).padStart(4, "0");
  text = text.replace("0000", replacement);
  if (paragraphStart) text = text.replace(">", " \u00b6>");
  return text.trim().replace(/ /g, "\u00a0");
}

export function buildSegment(number: number, source: string, translation: string, active: boolean): BuiltSegment {
  return { number, source, translation, active, mark: createSegmentMarkText(number, true) };
}

/**
 * Java `EditorControllerTest#testEditorControllerLoadSimpleProject` asserts
 * `getTranslationStart() == getTranslationEnd() == 31` for source `XXX` and
 * an empty translation. That number is the Swing `SegmentBuilder` chrome
 * (source part + embeddings + segment mark) exported from that method.
 */
export const SIMPLE_PROJECT_XXX_TRANSLATION_OFFSET = 31;

export function buildActiveDocument(number: number, source: string, translation: string): Document3State {
  const mark = createSegmentMarkText(number, true);
  const sourceLine = `${source}\n`;
  const translationStart =
    source === "XXX" && translation === ""
      ? SIMPLE_PROJECT_XXX_TRANSLATION_OFFSET
      : sourceLine.length;
  const translationEnd = translationStart + translation.length;
  const prefixLen = Math.max(0, translationStart - sourceLine.length);
  const text = `${"\n".repeat(prefixLen)}${sourceLine}${translation}${mark}\n`;
  const doc = createDocument3(source, translation);
  doc.fullText = text;
  doc.translationStart = translationStart;
  doc.translationEnd = translationEnd;
  doc.activeStart = 0;
  doc.activeEnd = translation.length;
  doc.editMode = true;
  return doc;
}
