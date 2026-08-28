/** Java `org.omegat.gui.editor.mark.CalcMarkersThread`. */
import type { IMarker, MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export function calcMarkers(markers: IMarker[], input: MarkerInput): Mark[] {
  const out: Mark[] = [];
  for (const m of markers) {
    const marks = m.getMarksForEntry(input);
    if (marks) out.push(...marks);
  }
  return out;
}
