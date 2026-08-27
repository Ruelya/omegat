/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterTableView`. */
import { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AutoCompleterTableView extends AbstractAutoCompleterView {
  row = 0;
  col = 0;
  cells: AutoCompleterItem[][] = [];
}
