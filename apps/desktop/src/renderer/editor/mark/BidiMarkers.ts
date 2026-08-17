/** Java `org.omegat.gui.editor.mark.BidiMarkers`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const BIDI = "\u200e\u200f\u202a\u202b\u202c\u202d\u202e\u2066\u2067\u2068\u2069";

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
