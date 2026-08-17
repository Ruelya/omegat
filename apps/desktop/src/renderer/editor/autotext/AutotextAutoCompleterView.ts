/** Java `org.omegat.gui.editor.autotext.AutotextAutoCompleterView`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { matchAutotext, parseAutotext, type AutotextEntry } from "./Autotext";

export class AutotextAutoCompleterView extends AutoCompleterListView {
  entries: AutotextEntry[] = [];
  constructor(raw = "") {
    super("autotext");
    this.entries = parseAutotext(raw);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    return matchAutotext(this.entries, wordChunk).map((e) => item(e.full, "autotext", [e.shortcut, e.comment]));
  }
}
