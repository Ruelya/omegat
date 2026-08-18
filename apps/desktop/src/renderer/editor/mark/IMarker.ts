/** Java `org.omegat.gui.editor.mark.IMarker`. */
import type { Mark } from "./Mark";

export type ProtectedPart = { text: string; tooltip?: string };

export type MarkerInput = {
  sourceText: string | null;
  translationText: string | null;
  isActive: boolean;
  displaySource?: boolean;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  enabled?: boolean;
  protectedParts?: ProtectedPart[];
};

export interface IMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null;
}
