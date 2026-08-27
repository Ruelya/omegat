/** Java `org.omegat.gui.editor.IEditor` — every method is implemented (no empty break). */

import { useApp } from "../store/app";
import { switchCase } from "../lib/editor-doc";
import {
  activateTranslation,
  commitAndDeactivate as deactivateDocument,
  type Document3State,
} from "./Document3";
import { EditorTextArea3 } from "./EditorTextArea3";
import { changeCase as changeEditorCase, getWordBoundary } from "./EditorUtils";

export type EditorFilter = { kind: "untranslated" | "unique" | "noted" | "none"; query?: string };

let filter: EditorFilter = { kind: "none" };
let selectedText = "";
const popupConstructors: Array<(x: number, y: number) => void> = [];
let remarkMarker: (name: string) => void = (_name) => undefined;

function sameActiveEntry(
  left: ReturnType<typeof useApp.getState>["entries"][number] | undefined,
  right: ReturnType<typeof useApp.getState>["entries"][number] | undefined,
): boolean {
  return Boolean(left && right && JSON.stringify(left.key) === JSON.stringify(right.key));
}

function commandTextArea(): EditorTextArea3 | null {
  const state = useApp.getState();
  const entry = state.entries[state.index];
  if (!entry) return null;
  const doc = state.document3;
  const limit = doc.translation.length;
  const anchor = Math.max(0, Math.min(state.editorSelection.anchor, limit));
  const focus = Math.max(0, Math.min(state.editorSelection.focus, limit));
  const area = new EditorTextArea3();
  area.setDocument(doc);
  area.setTargetLocale(state.props?.target_lang || "und");
  area.setSelection(doc.translationStart + anchor, doc.translationStart + focus);
  return area;
}

function publishCommandDocument(area: EditorTextArea3, previous: Document3State): void {
  const next = area.getOmDocument();
  if (next === previous) return;
  const state = useApp.getState();
  state.setDraft(next.translation);
  const translationStart = next.translationStart;
  useApp.setState({
    document3: next,
    editorSelection: {
      anchor: area.getSelectionAnchor() - translationStart,
      focus: area.getSelectionFocus() - translationStart,
    },
    selectedText: "",
  });
  selectedText = "";
}

export function bindMarkerRemark(fn: (name: string) => void): () => void {
  remarkMarker = fn;
  return () => {
    if (remarkMarker === fn) remarkMarker = () => undefined;
  };
}

export const IEditor = {
  activateEntry() {
    const a = useApp.getState();
    if (!a.entries[a.index]) return;
    const doc = a.document3;
    useApp.setState({
      document3: activateTranslation(doc, doc.translationStart, doc.translationEnd),
      editorSelection: { anchor: 0, focus: 0 },
      selectedText: "",
    });
    selectedText = "";
  },
  changeCase(mode: "upper" | "lower" | "title" | "sentence" | "cycle") {
    const area = commandTextArea();
    if (!area) return;
    const previous = area.getOmDocument();
    const doc = previous;
    if (area.getSelectionStart() === area.getSelectionEnd()) {
      const caret = Math.max(0, area.getCaretPosition() - doc.translationStart);
      const probe = caret > 0 ? caret - 1 : caret;
      const locale = useApp.getState().props?.target_lang || "und";
      area.setSelection(
        doc.translationStart + getWordBoundary(locale, doc.translation, probe, false),
        doc.translationStart + getWordBoundary(locale, doc.translation, probe, true),
      );
    }
    const replacement = changeEditorCase(
      area.getSelectedText(),
      mode,
      useApp.getState().props?.target_lang || "und",
    );
    if (replacement === area.getSelectedText()) return;
    area.replaceSelection(replacement);
    publishCommandDocument(area, previous);
  },
  async commitAndDeactivate() {
    const before = useApp.getState();
    const entry = before.entries[before.index];
    if (!entry) return;
    await before.commitCurrent();
    const after = useApp.getState();
    if (!sameActiveEntry(entry, after.entries[after.index])) return;
    useApp.setState({
      document3: deactivateDocument(after.document3),
      completer: [],
    });
  },
  async commitAndLeave() {
    const before = useApp.getState();
    const entry = before.entries[before.index];
    if (!entry) return;
    const selection = { ...before.editorSelection };
    await before.commitCurrent();
    const after = useApp.getState();
    if (!sameActiveEntry(entry, after.entries[after.index])) return;
    const limit = after.document3.translation.length;
    useApp.setState({
      document3: activateTranslation(
        after.document3,
        after.document3.translationStart,
        after.document3.translationEnd,
      ),
      editorSelection: {
        anchor: Math.max(0, Math.min(selection.anchor, limit)),
        focus: Math.max(0, Math.min(selection.focus, limit)),
      },
    });
  },
  getAutoCompleter() {
    return useApp.getState().completer;
  },
  getCurrentEntry() {
    const a = useApp.getState();
    return a.entries[a.index] ?? null;
  },
  getCurrentEntryNumber() {
    const a = useApp.getState();
    return a.entries[a.index] ? a.index + 1 : 0;
  },
  getCurrentFile() {
    return this.getCurrentEntry()?.file ?? "";
  },
  getCurrentPositionInEntryTranslationInEditor() {
    const a = useApp.getState();
    if (!a.document3.editMode) return { position: -1 };
    const { anchor, focus } = a.editorSelection;
    if (anchor === focus) return { position: focus };
    return {
      selectionStart: Math.min(anchor, focus),
      selectionEnd: Math.max(anchor, focus),
    };
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
    const a = useApp.getState();
    const start = Math.min(a.editorSelection.anchor, a.editorSelection.focus);
    const end = Math.max(a.editorSelection.anchor, a.editorSelection.focus);
    return start === end
      ? a.selectedText || selectedText
      : a.document3.translation.slice(start, end);
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
    const area = commandTextArea();
    if (!area) return;
    const previous = area.getOmDocument();
    if (area.replaceSelection(text)) publishCommandDocument(area, previous);
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
    const a = useApp.getState();
    a.redo();
    const end = useApp.getState().document3.translation.length;
    a.setEditorSelection({ anchor: end, focus: end });
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
  },
  removeFilter() {
    filter = { kind: "none" };
  },
  replaceEditText(text: string) {
    const a = useApp.getState();
    a.setDraft(text);
    a.setEditorSelection({ anchor: text.length, focus: text.length });
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
    const a = useApp.getState();
    a.undo();
    const end = useApp.getState().document3.translation.length;
    a.setEditorSelection({ anchor: end, focus: end });
  },
  windowDeactivated() {
    useApp.setState({ completer: [] });
  },
};

export function setSelectedText(s: string) {
  selectedText = s;
}

export { switchCase };
