/** Java `org.omegat.gui.editor.autocompleter.IAutoCompleter`. */
import type { AutoCompleterItem } from "./AutoCompleterItem";

export interface IAutoCompleter {
  updatePopup(): AutoCompleterItem[];
  confirm(): string | null;
  isVisible(): boolean;
}
