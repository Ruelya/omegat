/** Java `org.omegat.gui.editor.mark.ComesFromMTMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromMTMarker extends AbstractMarker {
  markedSte: string | null = null;
  markedText: string | null = null;

  setMark(ste: string | null, text: string | null) {
    this.markedSte = ste;
    this.markedText = text;
  }

  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!input.isActive) return null;
    if (input.sourceText == null && input.translationText == null) return null;
    if (input.translationText == null || input.translationText !== this.markedText) return null;
    return [mark(0, input.translationText.length, "mt", "MT")];
  }
}

export function comesFromMTMarker(fromMt: boolean, len: number) {
  const m = new ComesFromMTMarker();
  const text = "x".repeat(len);
  if (fromMt) m.setMark("ste", text);
  return (
    m.getMarksForEntry({
      sourceText: "source",
      translationText: text,
      isActive: true,
    }) ?? []
  );
}
