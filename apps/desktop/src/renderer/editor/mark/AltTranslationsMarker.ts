/** Java `org.omegat.gui.editor.mark.AltTranslationsMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class AltTranslationsMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !input.isAlt || !input.translationText) return null;
    return [mark(0, input.translationText.length, "alt", "ALT")];
  }
}

export function altTranslationsMarker(isAlt: boolean, len: number) {
  return isAlt && len > 0 ? [mark(0, len, "alt", "ALT")] : [];
}
