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
  linked?: "xICE" | "x100PC" | "xAUTO" | "xENFORCED";
  enabled?: boolean;
  protectedParts?: ProtectedPart[];
};

export interface IMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null;
}

export interface IAsyncMarker {
  getMarksForEntryAsync(input: MarkerInput): Promise<Mark[] | null>;
}

export type MarkerProvider = IMarker | IAsyncMarker;

export function isAsyncMarker(marker: MarkerProvider): marker is IAsyncMarker {
  return "getMarksForEntryAsync" in marker;
}
