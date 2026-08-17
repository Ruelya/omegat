/** Java `org.omegat.gui.editor.chartable.CharTableModel`. */
export class CharTableModel {
  constructor(public chars: string) {}
  cell(i: number): string {
    return this.chars[i] ?? "";
  }
  size() {
    return this.chars.length;
  }
}
