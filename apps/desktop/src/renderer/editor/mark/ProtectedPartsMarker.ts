/** Java `org.omegat.gui.editor.mark.ProtectedPartsMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput, ProtectedPart } from "./IMarker";

const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;
const PRINTF = /%(?:\d+\$)?[-+#0 ]*\d*(?:\.\d+)?[diouxXeEfFgGaAcspn%]/g;

export class ProtectedPartsMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    const parts = input.protectedParts ?? inferParts(input.sourceText);
    if (parts.length === 0) return null;
    if (input.sourceText == null && input.translationText == null) return null;
    const out: Mark[] = [];
    if (input.sourceText != null) collectParts(input.sourceText, parts, true, out);
    if (input.translationText != null) collectParts(input.translationText, parts, false, out);
    return out;
  }
}

function inferParts(source: string | null | undefined): ProtectedPart[] {
  if (!source) return [];
  const found: ProtectedPart[] = [];
  for (const re of [TAG, PRINTF]) {
    const copy = new RegExp(re.source, "g");
    for (const m of source.matchAll(copy)) {
      found.push({ text: m[0], tooltip: m[0] });
    }
  }
  return found;
}

function collectParts(text: string, parts: ProtectedPart[], source: boolean, out: Mark[]): void {
  for (const pp of parts) {
    let pos = -1;
    while ((pos = text.indexOf(pp.text, pos + 1)) >= 0) {
      out.push(mark(pos, pos + pp.text.length, "protected", pp.tooltip ?? pp.text, source));
    }
  }
}

export function protectedPartsMarker(text: string, parts?: ProtectedPart[]): Mark[] {
  return (
    new ProtectedPartsMarker().getMarksForEntry({
      sourceText: text,
      translationText: null,
      isActive: true,
      protectedParts: parts,
    }) ?? []
  );
}
