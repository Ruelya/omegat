/** Java `org.omegat.gui.editor.mark.ComesFromAutoTMMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromAutoTMMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.fromAuto || !input.translationText) return null;
    return [mark(0, input.translationText.length, "auto-tm", "AUTO_TM")];
  }
}

export function comesFromAutoTMMarker(fromAuto: boolean, len: number) {
  return fromAuto && len > 0 ? [mark(0, len, "auto-tm", "AUTO_TM")] : [];
}
