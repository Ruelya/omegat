/** Java `org.omegat.gui.editor.autocompleter.AbstractAutoCompleterView`. */
import type { AutoCompleterItem } from "./AutoCompleterItem";

export abstract class AbstractAutoCompleterView {
  constructor(public name: string) {}
  abstract computeListData(wordChunk: string, onlyCurrentWord: boolean): AutoCompleterItem[];
  shouldPopUp(wordChunk: string): boolean {
    return wordChunk.length > 0;
  }
}
