/** Java `org.omegat.gui.editor.mark.NBSPMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class NBSPMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.isActive ? input.translationText || input.sourceText : input.sourceText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      if (text[i] === "\u00a0") out.push(mark(i, i + 1, "nbsp", "NBSP"));
    }
    return out;
  }
}

export function nbspMarker(text: string) {
  return new NBSPMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
