/** Java `org.omegat.gui.editor.mark.AbstractMarker`. */
import type { IMarker, MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export abstract class AbstractMarker implements IMarker {
  enabled = true;
  abstract getMarksForEntry(input: MarkerInput): Mark[] | null;
  isEnabled(): boolean {
    return this.enabled;
  }
}
