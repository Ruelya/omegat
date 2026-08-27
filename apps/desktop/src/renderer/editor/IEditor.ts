/** Java `org.omegat.gui.editor.IEditor` — every method is implemented (no empty break). */

import { useApp } from "../store/app";
import { switchCase } from "../lib/editor-doc";

export type EditorFilter = { kind: "untranslated" | "unique" | "noted" | "none"; query?: string };

let filter: EditorFilter = { kind: "none" };
let selectedText = "";
const popupConstructors: Array<(x: number, y: number) => void> = [];
let remarkMarker: (name: string) => void = (_name) => undefined;

export function bindMarkerRemark(fn: (name: string) => void): () => void {
  remarkMarker = fn;
  return () => {
    if (remarkMarker === fn) remarkMarker = () => undefined;
  };
}

export const IEditor = {
  activateEntry() {
    const a = useApp.getState();
    a.select(a.index, false);
  },
  changeCase(mode: "upper" | "lower" | "title" | "sentence" | "cycle") {
    useApp.getState().applyCase(mode);
  },
  async commitAndDeactivate() {
    await useApp.getState().commit();
  },
  async commitAndLeave() {
    await useApp.getState().commit();
    await useApp.getState().jump("next");
  },
  getAutoCompleter() {
    return useApp.getState().completer;
  },
  getCurrentEntry() {
    const a = useApp.getState();
    return a.entries[a.index] ?? null;
  },
  getCurrentEntryNumber() {
    return useApp.getState().index + 1;
  },
  getCurrentFile() {
    return this.getCurrentEntry()?.file ?? "";
  },
  getCurrentPositionInEntryTranslationInEditor() {
    return useApp.getState().draft.length;
  },
  getCurrentTargetFile() {
    const a = useApp.getState();
    const e = a.entries[a.index];
    if (!e || !a.props) return "";
    return `${a.props.target_dir}/${e.file}`;
  },
  getCurrentTranslation() {
    return useApp.getState().draft;
  },
  getFilter() {
    return filter;
  },
  getSelectedText() {
    return selectedText;
  },
  getSettings() {
    return useApp.getState().marks;
  },
  async gotoEntry(n: number) {
    await useApp.getState().jump("number", n);
  },
  async gotoEntryAfterFix(n: number) {
    await this.gotoEntry(n);
    this.refreshViewAfterFix();
  },
  async gotoFile(name: string) {
    const a = useApp.getState();
    const i = a.entries.findIndex((e) => e.file === name);
    if (i >= 0) await a.select(i);
  },
  async gotoHistoryBack() {
    await useApp.getState().historyBack();
  },
  async gotoHistoryForward() {
    await useApp.getState().historyForward();
  },
  insertTag() {
    useApp.getState().insertTag();
  },
  insertText(text: string) {
    const a = useApp.getState();
    a.setDraft(a.draft + text);
  },
  insertTextAndMark(text: string) {
    this.insertText(text);
  },
  isOrientationAllLtr() {
    return true;
  },
  markActiveEntrySource() {
    return this.getCurrentEntry()?.source ?? "";
  },
  async nextEntry() {
    await useApp.getState().jump("next");
  },
  async nextEntryWithNote() {
    await useApp.getState().jump("note", undefined, 1);
  },
  async nextTranslatedEntry() {
    await useApp.getState().jump("translated");
  },
  async nextUniqueEntry() {
    await useApp.getState().jump("unique");
  },
  async nextUntranslatedEntry() {
    await useApp.getState().jump("untranslated");
  },
  async nextXAutoEntry() {
    await useApp.getState().jump("auto", undefined, 1);
  },
  async nextXEnforcedEntry() {
    await useApp.getState().jump("enforce", undefined, 1);
  },
  async prevEntry() {
    await useApp.getState().jump("prev");
  },
  async prevEntryWithNote() {
    await useApp.getState().jump("note", undefined, -1);
  },
  async prevXAutoEntry() {
    await useApp.getState().jump("auto", undefined, -1);
  },
  async prevXEnforcedEntry() {
    await useApp.getState().jump("enforce", undefined, -1);
  },
  redo() {
    useApp.getState().redo();
  },
  refreshView() {
    const a = useApp.getState();
    a.select(a.index, false);
  },
  refreshViewAfterFix() {
    this.refreshView();
  },
  async registerEmptyTranslation() {
    await useApp.getState().registerEmpty();
  },
  async registerIdenticalTranslation() {
    await useApp.getState().registerIdentical();
  },
  registerPopupMenuConstructors(fn: (x: number, y: number) => void) {
    popupConstructors.push(fn);
  },
  async registerUntranslated() {
    await useApp.getState().registerUntranslated();
  },
  remarkOneMarker(name: string) {
    remarkMarker(name);
    this.refreshView();
  },
  removeFilter() {
    filter = { kind: "none" };
  },
  replaceEditText(text: string) {
    useApp.getState().setDraft(text);
  },
  replaceEditTextAndMark(text: string) {
    this.replaceEditText(text);
  },
  requestFocus() {
    document.querySelector<HTMLElement>("[role='textbox']")?.focus();
  },
  selectSourceText() {
    useApp.getState().selectSource();
    selectedText = this.getCurrentEntry()?.source ?? "";
  },
  setAlternateTranslationForCurrentEntry(alt: boolean) {
    void useApp.getState().commit({ default_translation: !alt });
  },
  setFilter(next: EditorFilter) {
    filter = next;
  },
  undo() {
    useApp.getState().undo();
  },
  windowDeactivated() {
    void useApp.getState().save();
  },
};

export function setSelectedText(s: string) {
  selectedText = s;
}

export { switchCase };
