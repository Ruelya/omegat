/** Java `org.omegat.gui.editor.EditorTextArea3` headless interaction model. */
import {
  applyDocumentEdit,
  createDocument3,
  replaceEditText,
  setTextBeingComposed,
  type Document3State,
} from "./Document3";
import {
  deleteBackwardAtomic,
  deleteForwardAtomic,
  deleteRangeAtomic,
  moveCaret,
  snapCaret,
} from "../lib/editor-doc";

export type PopupMenuConstructor = {
  priority: number;
  build(position: number, activeEntry: boolean, activeTranslation: boolean): unknown[];
};

type CompositionSession = {
  snapshot: Document3State;
  start: number;
  originalAnchor: number;
  originalFocus: number;
  currentLength: number;
  currentText: string;
};

export class EditorTextArea3 {
  doc: Document3State;
  private caretPosition = 0;
  private selectionAnchor = 0;
  private selectionFocus = 0;
  private lockCursorToInputArea = true;
  private overtypeMode = false;
  private sourceLocale = "und";
  private targetLocale = "und";
  private currentWord: string | null = null;
  private composition: CompositionSession | null = null;
  private readonly popupConstructors: PopupMenuConstructor[] = [];
  private readonly wordListeners = new Set<(word: string | null, locale: string) => void>();

  constructor(source = "", translation = "") {
    this.doc = createDocument3(source, translation);
    this.caretPosition = this.doc.translationEnd;
    this.selectionAnchor = this.caretPosition;
    this.selectionFocus = this.caretPosition;
  }

  setDocument(doc: Document3State): void {
    this.composition = null;
    this.doc = doc;
    this.setCaretPosition(doc.translationEnd);
  }

  getOmDocument(): Document3State {
    return this.doc;
  }

  getText(): string {
    return this.doc.translation;
  }

  setText(text: string): void {
    this.doc = replaceEditText(this.doc, text);
    this.setCaretPosition(this.doc.translationEnd);
  }

  getCaretPosition(): number {
    return this.caretPosition;
  }

  setCaretPosition(position: number, bias: "before" | "after" = "after"): void {
    let next = Math.max(0, Math.min(position, this.doc.fullText.length));
    if (this.lockCursorToInputArea && this.doc.editMode) {
      next = Math.max(this.doc.translationStart, Math.min(next, this.doc.translationEnd));
    }
    const relative = next - this.doc.translationStart;
    if (relative >= 0 && relative <= this.doc.translation.length) {
      next = this.doc.translationStart + snapCaret(this.doc.translation, relative, bias);
    }
    this.caretPosition = next;
    this.selectionAnchor = next;
    this.selectionFocus = next;
    this.notifyWordAtCaret();
  }

  setSelection(start: number, end: number): void {
    this.setCaretPosition(start, "before");
    const anchor = this.caretPosition;
    this.setCaretPosition(end, "after");
    this.selectionAnchor = anchor;
    this.selectionFocus = this.caretPosition;
  }

  selectAll(): void {
    this.selectionAnchor = this.doc.translationStart;
    this.selectionFocus = this.doc.translationEnd;
    this.caretPosition = this.selectionFocus;
  }

  getSelectionStart(): number {
    return Math.min(this.selectionAnchor, this.selectionFocus);
  }

  getSelectionEnd(): number {
    return Math.max(this.selectionAnchor, this.selectionFocus);
  }

  getSelectedText(): string {
    return this.doc.fullText.slice(this.getSelectionStart(), this.getSelectionEnd());
  }

  replaceSelection(text: string): boolean {
    const start = this.getSelectionStart();
    const end = this.getSelectionEnd();
    const before = this.doc;
    this.doc = applyDocumentEdit(this.doc, start, end - start, text);
    if (this.doc === before) return false;
    this.setCaretPosition(start + text.length);
    return true;
  }

  insertText(text: string): boolean {
    if (this.getSelectionStart() !== this.getSelectionEnd()) return this.replaceSelection(text);
    const start = this.caretPosition;
    const remove = this.overtypeMode && start < this.doc.translationEnd ? 1 : 0;
    const before = this.doc;
    this.doc = applyDocumentEdit(this.doc, start, remove, text);
    if (this.doc === before) return false;
    this.setCaretPosition(start + text.length);
    return true;
  }

  beginComposition(): boolean {
    if (this.composition || !this.isInActiveTranslation(this.caretPosition)) return false;
    const start = this.getSelectionStart();
    const end = this.getSelectionEnd();
    this.composition = {
      snapshot: this.doc,
      start,
      originalAnchor: this.selectionAnchor,
      originalFocus: this.selectionFocus,
      currentLength: end - start,
      currentText: this.doc.fullText.slice(start, end),
    };
    this.doc = setTextBeingComposed(this.doc, true);
    return true;
  }

  updateComposition(text: string): boolean {
    const session = this.composition;
    if (!session) return false;
    if (session.currentText === text) return true;
    const before = this.doc;
    this.doc = applyDocumentEdit(this.doc, session.start, session.currentLength, text, {
      composed: true,
    });
    if (this.doc === before) return false;
    session.currentLength = text.length;
    session.currentText = text;
    this.caretPosition = session.start + text.length;
    this.selectionAnchor = this.caretPosition;
    this.selectionFocus = this.caretPosition;
    this.notifyWordAtCaret();
    return true;
  }

  commitComposition(text?: string): boolean {
    if (!this.composition) return false;
    if (text !== undefined && !this.updateComposition(text)) return false;
    this.doc = setTextBeingComposed(this.doc, false);
    this.composition = null;
    return true;
  }

  cancelComposition(): boolean {
    const session = this.composition;
    if (!session) return false;
    this.doc = setTextBeingComposed(session.snapshot, false);
    this.composition = null;
    this.selectionAnchor = session.originalAnchor;
    this.selectionFocus = session.originalFocus;
    this.caretPosition = session.originalFocus;
    this.notifyWordAtCaret();
    return true;
  }

  isComposing(): boolean {
    return this.composition !== null;
  }

  deleteBackward(): boolean {
    if (this.getSelectionStart() !== this.getSelectionEnd()) return this.replaceSelection("");
    const relative = this.caretPosition - this.doc.translationStart;
    const next = deleteBackwardAtomic(this.doc.translation, relative);
    if (next.text === this.doc.translation) return false;
    const removed = this.doc.translation.length - next.text.length;
    this.doc = applyDocumentEdit(this.doc, this.doc.translationStart + next.pos, removed, "");
    this.setCaretPosition(this.doc.translationStart + next.pos);
    return true;
  }

  deleteForward(): boolean {
    if (this.getSelectionStart() !== this.getSelectionEnd()) return this.replaceSelection("");
    const relative = this.caretPosition - this.doc.translationStart;
    const next = deleteForwardAtomic(this.doc.translation, relative);
    if (next.text === this.doc.translation) return false;
    const removed = this.doc.translation.length - next.text.length;
    this.doc = applyDocumentEdit(this.doc, this.doc.translationStart + next.pos, removed, "");
    this.setCaretPosition(this.doc.translationStart + next.pos);
    return true;
  }

  deleteSelectionAtomic(): boolean {
    const start = this.getSelectionStart() - this.doc.translationStart;
    const end = this.getSelectionEnd() - this.doc.translationStart;
    const next = deleteRangeAtomic(this.doc.translation, start, end);
    if (next.text === this.doc.translation) return false;
    const expandedLength = this.doc.translation.length - next.text.length;
    this.doc = applyDocumentEdit(
      this.doc,
      this.doc.translationStart + next.pos,
      expandedLength,
      "",
    );
    this.setCaretPosition(this.doc.translationStart + next.pos);
    return true;
  }

  moveCaret(direction: -1 | 1, extendSelection = false): number {
    const relative = this.caretPosition - this.doc.translationStart;
    const next = this.doc.translationStart + moveCaret(this.doc.translation, relative, direction);
    if (extendSelection) {
      this.caretPosition = next;
      this.selectionFocus = next;
      this.notifyWordAtCaret();
    } else {
      this.setCaretPosition(next, direction < 0 ? "before" : "after");
    }
    return this.caretPosition;
  }

  isInActiveTranslation(position: number): boolean {
    return (
      this.doc.editMode &&
      position >= this.doc.translationStart &&
      position <= this.doc.translationEnd
    );
  }

  toggleCursorLock(): boolean {
    this.lockCursorToInputArea = !this.lockCursorToInputArea;
    this.setCaretPosition(this.caretPosition);
    return this.lockCursorToInputArea;
  }

  isCursorLocked(): boolean {
    return this.lockCursorToInputArea;
  }

  toggleOvertype(): boolean {
    this.overtypeMode = !this.overtypeMode;
    return this.overtypeMode;
  }

  isOvertypeMode(): boolean {
    return this.overtypeMode;
  }

  setSourceLocale(locale: string): void {
    this.sourceLocale = locale || "und";
  }

  setTargetLocale(locale: string): void {
    this.targetLocale = locale || "und";
  }

  registerPopupMenuConstructor(constructor: PopupMenuConstructor): () => void {
    this.popupConstructors.push(constructor);
    this.popupConstructors.sort((a, b) => a.priority - b.priority);
    return () => {
      const index = this.popupConstructors.indexOf(constructor);
      if (index >= 0) this.popupConstructors.splice(index, 1);
    };
  }

  makePopupMenu(position = this.caretPosition): unknown[] {
    return this.popupConstructors.flatMap((constructor) =>
      constructor.build(position, true, this.isInActiveTranslation(position)),
    );
  }

  onCurrentWord(listener: (word: string | null, locale: string) => void): () => void {
    this.wordListeners.add(listener);
    return () => this.wordListeners.delete(listener);
  }

  private notifyWordAtCaret(): void {
    const inTarget = this.isInActiveTranslation(this.caretPosition);
    const text = inTarget ? this.doc.translation : this.doc.source;
    const relative = inTarget
      ? this.caretPosition - this.doc.translationStart
      : Math.min(this.caretPosition, text.length);
    const left = text.slice(0, relative).match(/[\p{L}\p{N}_]+$/u)?.[0] ?? "";
    const right = text.slice(relative).match(/^[\p{L}\p{N}_]+/u)?.[0] ?? "";
    const word = `${left}${right}` || null;
    if (word === this.currentWord) return;
    this.currentWord = word;
    const locale = inTarget ? this.targetLocale : this.sourceLocale;
    for (const listener of this.wordListeners) listener(word, locale);
  }
}
