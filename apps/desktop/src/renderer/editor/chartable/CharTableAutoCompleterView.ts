/** Java `org.omegat.gui.editor.chartable.CharTableAutoCompleterView`. */
import { AutoCompleterTableView } from "../autocompleter/AutoCompleterTableView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { CharTableModel } from "./CharTableModel";

export class CharTableAutoCompleterView extends AutoCompleterTableView {
  model: CharTableModel;
  constructor(chars = "") {
    super("chartable");
    this.model = new CharTableModel(chars);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return [...this.model.chars]
      .filter((c) => !p || c.toLowerCase().includes(p))
      .map((c) => item(c, "chartable"));
  }
}
