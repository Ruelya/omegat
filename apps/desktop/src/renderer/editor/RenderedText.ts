import { decorateText, parseDocument, type ViewMarks } from "../lib/editor-doc";
import { tooltipTextsOverRange } from "./MarkerController";
import type { EntryPart, Mark } from "./mark/Mark";

export type RenderedTextFragment = {
  text: string;
  classes: string[];
  offset: number;
  sourceLength: number;
  atomic: boolean;
  tooltipTexts: string[];
  tag?: string;
};

function marksOverlapping(
  marks: readonly Mark[],
  entryPart: EntryPart,
  start: number,
  end: number,
): Mark[] {
  return marks.filter((mark) =>
    mark.entryPart === entryPart
    && mark.startOffset < end
    && mark.endOffset > start
  );
}

function markerClasses(marks: readonly Mark[]): string[] {
  return [...new Set(marks.map((mark) =>
    mark.painter === "spell"
      ? "mark-spell"
      : `product-marker-${mark.painter.replace(/[^a-z0-9_-]/gi, "-")}`,
  ))];
}

/**
 * Build the renderer's model-aware text fragments. Every fragment records
 * the exact UTF-16 source interval it represents, independently of visible
 * whitespace/BiDi glyph expansion and overlapping Marker decorations.
 */
export function buildRenderedTextFragments(
  text: string,
  offset: number,
  viewMarks: ViewMarks,
  terms: readonly string[],
  productMarks: readonly Mark[] = [],
  entryPart: EntryPart = "TRANSLATION",
): RenderedTextFragment[] {
  const fragments: RenderedTextFragment[] = [];
  let cursor = offset;

  for (const token of parseDocument(text)) {
    const tokenStart = cursor;
    const tokenEnd = tokenStart + token.value.length;
    cursor = tokenEnd;
    if (token.kind === "tag") {
      const overlapping = marksOverlapping(productMarks, entryPart, tokenStart, tokenEnd);
      fragments.push({
        text: token.value,
        classes: ["tag", "tag-protected", ...markerClasses(overlapping)],
        offset: tokenStart,
        sourceLength: token.value.length,
        atomic: true,
        tooltipTexts: tooltipTextsOverRange(overlapping, entryPart, tokenStart, tokenEnd),
        tag: token.value,
      });
      continue;
    }

    const relevant = marksOverlapping(productMarks, entryPart, tokenStart, tokenEnd);
    const boundaries = new Set([0, token.value.length]);
    for (const mark of relevant) {
      boundaries.add(Math.max(0, mark.startOffset - tokenStart));
      boundaries.add(Math.min(token.value.length, mark.endOffset - tokenStart));
    }
    const points = [...boundaries].sort((a, b) => a - b);
    for (let sliceIndex = 0; sliceIndex < points.length - 1; sliceIndex += 1) {
      const sliceStart = points[sliceIndex]!;
      const sliceEnd = points[sliceIndex + 1]!;
      if (sliceEnd <= sliceStart) continue;
      const modelStart = tokenStart + sliceStart;
      const modelEnd = tokenStart + sliceEnd;
      const sliceMarks = marksOverlapping(relevant, entryPart, modelStart, modelEnd);
      const classes = markerClasses(sliceMarks);
      let decoratedOffset = modelStart;
      for (const decorated of decorateText(
        token.value.slice(sliceStart, sliceEnd),
        viewMarks,
        [...terms],
      )) {
        fragments.push({
          text: decorated.text,
          classes: [...decorated.cls, ...classes],
          offset: decoratedOffset,
          sourceLength: decorated.sourceLength,
          atomic:
            decorated.sourceLength !== decorated.text.length
            || sliceMarks.some((mark) => mark.painter === "protected"),
          tooltipTexts: tooltipTextsOverRange(
            sliceMarks,
            entryPart,
            decoratedOffset,
            decoratedOffset + decorated.sourceLength,
          ),
        });
        decoratedOffset += decorated.sourceLength;
      }
    }
  }
  return fragments;
}
