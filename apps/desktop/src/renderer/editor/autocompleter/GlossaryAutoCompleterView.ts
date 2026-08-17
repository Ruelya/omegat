/** Java glossary autocompleter view. */
import { AutoCompleterListView } from "./AutoCompleterListView";
import { item, type AutoCompleterItem } from "./AutoCompleterItem";

export class GlossaryAutoCompleterView extends AutoCompleterListView {
  terms: { source: string; target: string }[] = [];
  constructor() {
    super("glossary");
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return this.terms
      .filter((t) => t.source.toLowerCase().includes(p) || t.target.toLowerCase().includes(p))
      .map((t) => item(t.target, "glossary", [t.source]));
  }
}
