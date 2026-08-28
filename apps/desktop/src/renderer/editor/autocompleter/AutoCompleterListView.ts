/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterListView`. */
import { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AutoCompleterListView extends AbstractAutoCompleterView {
  selected = 0;
  items: AutoCompleterItem[] = [];
  select(i: number) {
    this.selected = Math.max(0, Math.min(i, this.items.length - 1));
  }
}
