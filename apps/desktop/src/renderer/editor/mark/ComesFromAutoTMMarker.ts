/** Java `org.omegat.gui.editor.mark.ComesFromAutoTMMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromAutoTMMarker extends AbstractMarker {
  markAutoPopulated = true;

  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.markAutoPopulated) return null;
    if (input.sourceText == null && input.translationText == null) return null;
    if (!input.fromAuto || input.translationText == null) return null;
    return [mark(0, input.translationText.length, "auto-tm", "AUTO_TM")];
  }
}

export function comesFromAutoTMMarker(fromAuto: boolean, len: number) {
  return (
    new ComesFromAutoTMMarker().getMarksForEntry({
      sourceText: fromAuto ? "Edit" : "",
      translationText: fromAuto ? "x".repeat(len) : null,
      isActive: true,
      fromAuto,
    }) ?? []
  );
}
