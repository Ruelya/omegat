/** Java `org.omegat.gui.editor.filter.FilterBarReplace`. */
import { applyReplace, type ReplaceFilter } from "./ReplaceFilter";

export function filterBarReplace(text: string, f: ReplaceFilter): string {
  return applyReplace(text, f);
}
