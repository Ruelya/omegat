/** Java `org.omegat.gui.editor.TagAutoCompleterView`. */
import { AutoCompleterListView } from "./autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "./autocompleter/AutoCompleterItem";

export class TagAutoCompleterView extends AutoCompleterListView {
  tags: string[] = [];
  constructor(tags: string[] = []) {
    super("tag");
    this.tags = tags;
  }
  computeListData(wordChunk: string, _contextualOnly = false): AutoCompleterItem[] {
    const p = wordChunk.toLowerCase();
    return this.tags.filter((t) => t.toLowerCase().includes(p)).map((t) => item(t, "tag"));
  }
}
