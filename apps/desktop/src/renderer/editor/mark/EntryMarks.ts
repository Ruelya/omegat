/** Java `org.omegat.gui.editor.mark.EntryMarks`. */
import type { Mark } from "./Mark";

export type EntryMarks = {
  sourceMarks: Mark[];
  translationMarks: Mark[];
};

export function emptyEntryMarks(): EntryMarks {
  return { sourceMarks: [], translationMarks: [] };
}
