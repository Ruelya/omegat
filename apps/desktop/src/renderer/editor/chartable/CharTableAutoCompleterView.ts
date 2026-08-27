/** Java `org.omegat.gui.editor.chartable.CharTableAutoCompleterView`. */
import { AutoCompleterTableView } from "../autocompleter/AutoCompleterTableView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { CharTableModel } from "./CharTableModel";

export class CharTableAutoCompleterView extends AutoCompleterTableView {
  model: CharTableModel;
  selection = { x: 0, y: 0 };

  constructor(chars: string | null = null) {
    super("chartable");
    this.model = new CharTableModel(chars);
    this.setSelection({ x: 0, y: 0 });
  }

  computeListData(wordChunk: string, _contextualOnly = false): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    const data = this.model.getData();
    if (!data) {
      const selected = this.getSelectedValue();
      return selected ? [selected] : [];
    }
    return [...data]
      .filter((c) => !p || c.toLowerCase().includes(p))
      .map((c) => item(c, "chartable"));
  }

  setSelection(p: { x: number; y: number }): void {
    this.selection = this.model.modifyPoint(p.x, p.y);
  }

  getSelectedValue(): AutoCompleterItem | null {
    const ch = this.model.getValueAt(this.selection.y, this.selection.x);
    return ch == null ? null : item(ch, "chartable");
  }

  shouldPopUp(): boolean {
    return false;
  }
}
