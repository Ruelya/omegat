/** Java `org.omegat.gui.editor.autotext.AutotextTableModel`. */
import type { AutotextEntry } from "./Autotext";

export class AutotextTableModel {
  constructor(public rows: AutotextEntry[] = []) {}
  getRowCount() {
    return this.rows.length;
  }
  getValueAt(row: number, col: number): string {
    const r = this.rows[row];
    if (!r) return "";
    return col === 0 ? r.shortcut : col === 1 ? r.full : r.comment;
  }
}
