/** Java `org.omegat.gui.editor.mark.WhitespaceMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class WhitespaceMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || input.sourceText == null) return null;
    const out: Mark[] = [];
    const markSource = input.isActive || input.displaySource || input.translationText == null;
    if (markSource) collectWs(input.sourceText, true, out);
    if (input.translationText != null) collectWs(input.translationText, false, out);
    return out;
  }
}

function collectWs(text: string, source: boolean, out: Mark[]): void {
  for (let i = 0; i < text.length; i++) {
    const ch = text[i]!;
    if (ch === " ") out.push(mark(i, i + 1, "·", undefined, source));
    else if (ch === "\t") out.push(mark(i, i + 1, "»", "Tab", source));
    else if (ch === "\n") out.push(mark(i, i + 1, "¶", "LF", source));
  }
}

export function whitespaceMarker(text: string): Mark[] {
  return (
    new WhitespaceMarker().getMarksForEntry({
      sourceText: text,
      translationText: null,
      isActive: true,
    }) ?? []
  );
}
