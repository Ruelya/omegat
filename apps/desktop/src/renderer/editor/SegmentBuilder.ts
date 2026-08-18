/** Java `org.omegat.gui.editor.SegmentBuilder`. */
import { createDocument3, StyledDocument, type Document3State } from "./Document3";

export type BuiltSegment = {
  source: string;
  translation: string;
  active: boolean;
  number: number;
  mark: string;
};

/** Java `OStrings.getSegmentMarker()` / `TF_CUR_SEGMENT_START`. */
export const TF_CUR_SEGMENT_START = "segment 0000";
/** Java `OConsts.SEGMENT_MARKER_STRING`. */
export const SEGMENT_MARKER_STRING = `<${TF_CUR_SEGMENT_START}>`;

export const BIDI_LRE = "\u202a";
export const BIDI_RLE = "\u202b";
export const BIDI_PDF = "\u202c";
export const BIDI_LRM = "\u200e";
export const BIDI_RLM = "\u200f";

export type SegmentBuilderOptions = {
  hasRTL?: boolean;
  sourceLangIsRTL?: boolean;
  targetLangIsRTL?: boolean;
  localeRtl?: boolean;
  displaySegmentSources?: boolean;
  dontInsertSourceText?: boolean;
  paragraphStart?: boolean;
};

/**
 * Java `SegmentBuilder.createSegmentMarkText`: replace `0000`, optional
 * paragraph `¶`, then trim and turn spaces into NBSP.
 */
export function createSegmentMarkText(segmentNumberInProject: number, paragraphStart = false): string {
  let text = SEGMENT_MARKER_STRING;
  const replacement = String(segmentNumberInProject).padStart(4, "0");
  if (text.includes("0000")) text = text.replace("0000", replacement);
  if (paragraphStart) text = text.replace(">", " \u00b6>");
  return text.trim().replace(/ /g, "\u00a0");
}

export function buildSegment(number: number, source: string, translation: string, active: boolean): BuiltSegment {
  return { number, source, translation, active, mark: createSegmentMarkText(number, true) };
}

class SegmentWriter {
  offset: number;
  constructor(
    readonly doc: StyledDocument,
    public readonly opts: Required<SegmentBuilderOptions>,
    start = 0,
  ) {
    this.offset = start;
  }

  insert(text: string): void {
    this.doc.insertString(this.offset, text);
    this.offset += text.length;
  }

  insertDirectionEmbedding(isRTL: boolean): void {
    if (this.opts.hasRTL) this.insert(isRTL ? BIDI_RLE : BIDI_LRE);
  }

  insertDirectionEndEmbedding(): void {
    if (this.opts.hasRTL) this.insert(BIDI_PDF);
  }

  insertDirectionMarker(isRTL: boolean): void {
    if (this.opts.hasRTL) this.insert(isRTL ? BIDI_RLM : BIDI_LRM);
  }

  addInactiveSegPart(isSource: boolean, text: string): string {
    const rtl = isSource ? this.opts.sourceLangIsRTL : this.opts.targetLangIsRTL;
    this.insertDirectionEmbedding(rtl);
    this.insert(text);
    this.insertDirectionEndEmbedding();
    this.insert("\n");
    return text;
  }

  addActiveSegPart(text: string, segmentNumber: number, paragraphStart: boolean): { begin: number; end: number } {
    const rtl = this.opts.targetLangIsRTL;
    this.insertDirectionEmbedding(rtl);
    const begin = this.offset;
    this.insert(text);
    const end = this.offset;
    this.insertDirectionEndEmbedding();
    this.insertDirectionMarker(rtl);
    this.insertDirectionEmbedding(this.opts.localeRtl);
    this.insert(createSegmentMarkText(segmentNumber, paragraphStart));
    this.insertDirectionEndEmbedding();
    this.insertDirectionMarker(rtl);
    this.insert("\n");
    return { begin, end };
  }
}

function defaults(opts?: SegmentBuilderOptions): Required<SegmentBuilderOptions> {
  return {
    hasRTL: opts?.hasRTL ?? false,
    sourceLangIsRTL: opts?.sourceLangIsRTL ?? false,
    targetLangIsRTL: opts?.targetLangIsRTL ?? false,
    localeRtl: opts?.localeRtl ?? false,
    displaySegmentSources: opts?.displaySegmentSources ?? true,
    dontInsertSourceText: opts?.dontInsertSourceText ?? true,
    paragraphStart: opts?.paragraphStart ?? true,
  };
}

/**
 * Java `EditorController.loadDocument` then `activateEntry` for one STE.
 *
 * Inactive first: source part + segment separator. Activate replaces that
 * span via `Document.remove` + `insertString` (trusted), then
 * `addInactiveSegPart(source)` + `addActiveSegPart(translation)`.
 *
 * `EditorControllerTest` is `@Assume !headless`, so CI cannot re-export 31
 * from Swing. The LTR empty-translation offset is whatever this insertString
 * model produces (source `XXX` + newline = 4).
 */
export function buildActiveDocument(
  number: number,
  source: string,
  translation: string,
  opts?: SegmentBuilderOptions,
): Document3State {
  const o = defaults(opts);
  const swing = new StyledDocument();

  // loadDocument: inactive element + separator (Java addSegmentSeparator).
  const inactive = new SegmentWriter(swing, o, 0);
  if (o.displaySegmentSources) inactive.addInactiveSegPart(true, source);
  const inactiveBegin = 0;
  const inactiveEnd = inactive.offset;
  swing.insertString(swing.getLength(), "\n");

  // activateEntry: remove the inactive span, rebuild as active at that offset.
  swing.remove(inactiveBegin, inactiveEnd - inactiveBegin);
  const active = new SegmentWriter(swing, o, inactiveBegin);
  active.addInactiveSegPart(true, source);
  let text = translation;
  if (!text && !o.dontInsertSourceText) text = source;
  const range = active.addActiveSegPart(text, number, o.paragraphStart);

  const doc = createDocument3(source, translation);
  doc.fullText = swing.text;
  doc.translationStart = range.begin;
  doc.translationEnd = range.end;
  doc.activeStart = 0;
  doc.activeEnd = translation.length;
  doc.editMode = true;
  return doc;
}
