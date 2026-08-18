/** Java `org.omegat.gui.editor.mark.NBSPMarker` — U+00A0 / U+202F / U+2007. */
import { AbstractMarker } from "./AbstractMarker";
import type { MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export class NBSPMarker extends AbstractMarker {
  constructor() {
    super();
    this.pattern = /[\u00a0\u202f\u2007]/g;
    this.toolTip = "NBSP";
    this.painter = "nbsp";
  }
}

export function nbspMarker(text: string): Mark[] {
  return (
    new NBSPMarker().getMarksForEntry({
      sourceText: text,
      translationText: null,
      isActive: true,
    }) ?? []
  );
}

export function nbspMarksForEntry(input: MarkerInput): Mark[] | null {
  return new NBSPMarker().getMarksForEntry(input);
}
