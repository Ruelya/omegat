/** Java `org.omegat.gui.editor.mark.RemoveTagMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import type { MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export const MARKER_REMOVETAG = "Text to remove";

export class RemoveTagMarker extends AbstractMarker {
  constructor() {
    super();
    this.pattern = /%remove/g;
    this.toolTip = MARKER_REMOVETAG;
    this.painter = "remove-tag";
  }

  isEnabled(): boolean {
    return true;
  }

  getMarksForEntry(input: MarkerInput): Mark[] | null {
    return super.getMarksForEntry(input);
  }
}

export function removeTagMarker(source: string, target: string) {
  return (
    new RemoveTagMarker().getMarksForEntry({
      sourceText: source,
      translationText: target || null,
      isActive: true,
    }) ?? []
  );
}
