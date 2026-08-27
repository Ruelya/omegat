/** Java `javax.swing.text.DocumentFilter.FilterBypass` used by `DocumentFilter3`. */
import type { FilterDocument } from "./DocumentFilter3";

export class FilterBypass {
  constructor(public doc: FilterDocument) {}

  getDocument(): FilterDocument {
    return this.doc;
  }

  replace(offset: number, length: number, text: string): void {
    const written = this.doc.text.slice(0, offset) + text + this.doc.text.slice(offset + length);
    const delta = text.length - length;
    this.doc = {
      ...this.doc,
      text: written,
      translationEnd: this.doc.translationEnd + delta,
    };
  }

  insertString(offset: number, text: string): void {
    this.replace(offset, 0, text);
  }

  remove(offset: number, length: number): void {
    this.replace(offset, length, "");
  }
}
