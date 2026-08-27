/** Java `org.omegat.gui.editor.autocompleter.AutoCompleterItem`. */
export type AutoCompleterItem = {
  payload: string;
  extras: string[];
  cursorAdjust: number;
  kind: string;
};

export function item(payload: string, kind: string, extras: string[] = [], cursorAdjust = 0): AutoCompleterItem {
  return { payload, extras, cursorAdjust, kind };
}
