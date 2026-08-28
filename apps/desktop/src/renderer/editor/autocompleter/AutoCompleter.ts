/** Java `org.omegat.gui.editor.autocompleter.AutoCompleter`. */
import type { AbstractAutoCompleterView } from "./AbstractAutoCompleterView";
import type { AutoCompleterItem } from "./AutoCompleterItem";
import { AutoCompleterKeys } from "./AutoCompleterKeys";
import { GlossaryAutoCompleterView } from "./GlossaryAutoCompleterView";
import type { IAutoCompleter } from "./IAutoCompleter";

export class AutoCompleter implements IAutoCompleter {
  views: AbstractAutoCompleterView[] = [new GlossaryAutoCompleterView()];
  viewIndex = 0;
  visible = false;
  items: AutoCompleterItem[] = [];
  selected = 0;

  setViews(views: AbstractAutoCompleterView[]) {
    this.views = views;
    this.viewIndex = 0;
  }

  updatePopup(wordChunk = ""): AutoCompleterItem[] {
    const view = this.views[this.viewIndex];
    this.items = view ? view.computeListData(wordChunk, true) : [];
    if (view && this.items.length === 0) {
      this.items = view.computeListData(wordChunk, false);
    }
    this.visible = this.items.length > 0;
    this.selected = 0;
    return this.items;
  }

  confirm(): string | null {
    const it = this.items[this.selected];
    this.visible = false;
    return it?.payload ?? null;
  }

  isVisible(): boolean {
    return this.visible;
  }

  nextView() {
    if (!this.views.length) return;
    this.viewIndex = (this.viewIndex + 1) % this.views.length;
  }

  keys() {
    return AutoCompleterKeys;
  }
}
