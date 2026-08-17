/** Java `org.omegat.gui.editor.mark.IMarker`. */
import type { Mark } from "./Mark";

export type MarkerInput = {
  sourceText: string;
  translationText: string;
  isActive: boolean;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  enabled?: boolean;
};

export interface IMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null;
}
