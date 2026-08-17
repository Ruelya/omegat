/** Java `org.omegat.gui.editor.mark.WhitespaceMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class WhitespaceMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.sourceText) return null;
    const text = input.isActive || !input.translationText ? input.sourceText : input.translationText;
    const source = input.isActive || !input.translationText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      const ch = text[i]!;
      if (ch === " ") out.push(mark(i, i + 1, "·", "SPACE", source));
      else if (ch === "\t") out.push(mark(i, i + 1, "»", "TAB", source));
      else if (ch === "\n") out.push(mark(i, i + 1, "¶", "LF", source));
    }
    return out;
  }
}

export function whitespaceMarker(text: string) {
  return new WhitespaceMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
