/** Java `org.omegat.gui.editor.chartable.CharTableModel`. */

export const ZERO_WIDTH_SPACE = "\u200b";
const DEFAULT_GLYPH_START = 32;
const DEFAULT_GLYPH_END = 0xfff;
const DEFAULT_GLYPH_COUNT = DEFAULT_GLYPH_END - DEFAULT_GLYPH_START;
const EXTRA_DEFAULT_GLYPHS = [ZERO_WIDTH_SPACE];

export class CharTableModel {
  columnCount = 16;
  data: string | null;
  glyphCount: number;

  constructor(data: string | null = null) {
    this.data = null;
    this.glyphCount = DEFAULT_GLYPH_COUNT + EXTRA_DEFAULT_GLYPHS.length;
    this.setData(data);
  }

  setData(data: string | null): boolean {
    if (this.data == null && data == null) return false;
    if (data != null && this.data != null && this.data === data) return false;
    if (data == null) {
      this.glyphCount = DEFAULT_GLYPH_COUNT + EXTRA_DEFAULT_GLYPHS.length;
      this.data = null;
    } else {
      this.glyphCount = data.length;
      this.data = data;
    }
    return true;
  }

  getData(): string {
    return this.data ?? "";
  }

  get chars(): string {
    return this.data ?? "";
  }

  allowOnlyUnique(): void {
    if (this.data == null) return;
    let temp = "";
    for (const ch of this.data) {
      if (!temp.includes(ch)) temp += ch;
    }
    this.data = temp;
    this.glyphCount = temp.length;
  }

  appendChar(c: string, checkUnique: boolean): void {
    if (this.data == null) this.data = "";
    if (checkUnique && this.data.includes(c)) return;
    this.data += c;
    this.glyphCount = this.data.length;
  }

  removeSelection(row1: number, col1: number, row2: number, col2: number): void {
    if (!this.data || this.data.length === 0) return;
    let pos1 = row1 * this.columnCount + col1;
    pos1 = pos1 >= this.data.length ? this.data.length - 1 : pos1;
    let pos2 = row2 * this.columnCount + col2;
    pos2 = pos2 >= this.data.length ? this.data.length - 1 : pos2;
    if (pos2 === pos1) pos2 = pos1 + 1;
    this.data = this.data.slice(0, pos1) + this.data.slice(pos2);
    this.glyphCount = this.data.length;
  }

  getRowCount(): number {
    return Math.floor(this.glyphCount / this.columnCount) + (this.glyphCount % this.columnCount > 0 ? 1 : 0);
  }

  getColumnCount(): number {
    return this.columnCount;
  }

  getValueAt(rowIndex: number, columnIndex: number): string | null {
    const value = rowIndex * this.columnCount + columnIndex;
    if (value < this.glyphCount) {
      if (this.data != null) return this.data[value] ?? null;
      if (value < DEFAULT_GLYPH_COUNT) return String.fromCharCode(value + DEFAULT_GLYPH_START);
      return EXTRA_DEFAULT_GLYPHS[value - DEFAULT_GLYPH_COUNT] ?? null;
    }
    return null;
  }

  cell(i: number): string {
    return this.data?.[i] ?? "";
  }

  size(): number {
    return this.glyphCount;
  }

  modifyPoint(x: number, y: number): { x: number; y: number } {
    if (y * this.columnCount + x >= this.glyphCount) {
      const g = this.glyphCount === 0 ? 0 : this.glyphCount - 1;
      return { x: g % this.columnCount, y: Math.floor(g / this.columnCount) };
    }
    return { x, y };
  }
}
