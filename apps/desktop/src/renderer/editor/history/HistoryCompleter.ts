/** Java `org.omegat.gui.editor.history.HistoryCompleter`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { completeWords } from "./WordCompleter";

export class HistoryCompleter extends AutoCompleterListView {
  translations: string[] = [];
  constructor() {
    super("history");
  }
  computeListData(wordChunk: string, _contextualOnly = false): AutoCompleterItem[] {
    return completeWords(this.translations, wordChunk).map((w) => item(w, "history"));
  }
}
