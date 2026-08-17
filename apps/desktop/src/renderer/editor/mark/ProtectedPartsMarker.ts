/** Java `org.omegat.gui.editor.mark.ProtectedPartsMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;

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
