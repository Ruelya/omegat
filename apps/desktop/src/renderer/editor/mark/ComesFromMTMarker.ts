/** Java `org.omegat.gui.editor.mark.ComesFromMTMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromMTMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.fromMt || !input.translationText) return null;
    return [mark(0, input.translationText.length, "mt", "MT")];
  }
}

export function comesFromMTMarker(fromMt: boolean, len: number) {
  return fromMt && len > 0 ? [mark(0, len, "mt", "MT")] : [];
}
