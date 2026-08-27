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
import { getWordBoundary, removeDirectionChars } from "./EditorUtils";

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

export type SelectionDirection = "forward" | "backward" | "none";

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
  private currentWordLocale = "und";
  private focused = false;
  private composition: CompositionSession | null = null;
  private readonly popupConstructors: PopupMenuConstructor[] = [];
  private readonly wordListeners = new Set<(word: string | null, locale: string) => void>();
  private readonly focusListeners = new Set<(focused: boolean) => void>();

  constructor(source = "", translation = "") {
    this.doc = createDocument3(source, translation);
    this.caretPosition = this.doc.translationEnd;
    this.selectionAnchor = this.caretPosition;
    this.selectionFocus = this.caretPosition;
  }

  setDocument(doc: Document3State, preserveSelection = false): void {
    const anchor = this.selectionAnchor - this.doc.translationStart;
    const focus = this.selectionFocus - this.doc.translationStart;
    this.composition = null;
    this.doc = doc;
    if (preserveSelection) {
      this.setSelection(
        doc.translationStart + Math.max(0, Math.min(anchor, doc.translation.length)),
        doc.translationStart + Math.max(0, Math.min(focus, doc.translation.length)),
      );
    } else {
      this.setCaretPosition(doc.translationEnd);
    }
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
    const next = this.normalizeCaretPosition(position, bias);
    this.caretPosition = next;
    this.selectionAnchor = next;
    this.selectionFocus = next;
    this.notifyWordAtCaret();
  }

  /**
   * Apply a UTF-16 offset resolved from Electron's native caret hit-testing.
   * Tag interiors are snapped atomically according to the pixel-side bias.
   */
  setCaretFromRenderedOffset(
    offset: number,
    bias: "before" | "after" = "after",
    extendSelection = false,
  ): number {
    const relative = Math.max(0, Math.min(offset, this.doc.translation.length));
    const next = this.normalizeCaretPosition(this.doc.translationStart + relative, bias);
    if (extendSelection) {
      this.caretPosition = next;
      this.selectionFocus = next;
      this.notifyWordAtCaret();
    } else {
      this.setCaretPosition(next, bias);
    }
    return this.caretPosition;
  }

  setSelection(start: number, end: number): void {
    this.setCaretPosition(start, "before");
    const anchor = this.caretPosition;
    this.setCaretPosition(end, "after");
    this.selectionAnchor = anchor;
    this.selectionFocus = this.caretPosition;
  }

  getSelectionDirection(): SelectionDirection {
    if (this.selectionAnchor === this.selectionFocus) return "none";
    return this.selectionAnchor < this.selectionFocus ? "forward" : "backward";
  }

  getSelectionAnchor(): number {
    return this.selectionAnchor;
  }

  getSelectionFocus(): number {
    return this.selectionFocus;
  }

  collapseSelection(to: "start" | "end" = "end"): void {
    this.setCaretPosition(to === "start" ? this.getSelectionStart() : this.getSelectionEnd());
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
    return removeDirectionChars(
      this.doc.fullText.slice(this.getSelectionStart(), this.getSelectionEnd()),
    );
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
    const remove = this.overtypeMode
      ? Math.min(text.length, this.doc.translationEnd - start)
      : 0;
    const before = this.doc;
    this.doc = applyDocumentEdit(this.doc, start, remove, text);
    if (this.doc === before) return false;
    this.setCaretPosition(start + text.length);
    return true;
  }

  /**
   * Route Chromium/Electron `beforeinput` operations through Document3 instead
   * of synthesizing printable characters from keydown events.
   */
  handleBeforeInput(inputType: string, data: string | null = null): boolean {
    switch (inputType) {
      case "insertText":
        if (this.composition) {
          this.commitComposition(data ?? undefined);
        } else if (data !== null) {
          this.insertText(data);
        }
        return true;
      case "insertReplacementText":
      case "insertFromDrop":
      case "insertFromPaste":
      case "insertFromYank":
        if (data !== null) this.insertText(data);
        return true;
      case "insertLineBreak":
      case "insertParagraph":
        this.insertText("\n");
        return true;
      case "deleteContentBackward":
        this.deleteBackward();
        return true;
      case "deleteContentForward":
        this.deleteForward();
        return true;
      case "deleteWordBackward":
      case "deleteSoftLineBackward":
      case "deleteHardLineBackward":
        this.deleteToken(-1);
        return true;
      case "deleteWordForward":
      case "deleteSoftLineForward":
      case "deleteHardLineForward":
        this.deleteToken(1);
        return true;
      case "deleteContent":
      case "deleteByCut":
      case "deleteByDrag":
        this.deleteSelectionAtomic();
        return true;
      case "insertCompositionText":
        if (!this.composition) this.beginComposition();
        if (this.composition) this.updateComposition(data ?? "");
        return true;
      case "deleteCompositionText":
        if (this.composition) this.updateComposition("");
        return true;
      case "insertFromComposition":
        if (this.composition) {
          this.commitComposition(data ?? undefined);
        } else if (data !== null) {
          this.insertText(data);
        }
        return true;
      default:
        return false;
    }
  }

  beginComposition(): boolean {
    this.clampSelectionToTranslation();
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

  moveByToken(direction: -1 | 1, extendSelection = false): number {
    const inTarget = this.isInActiveTranslation(this.caretPosition);
    if (!inTarget) return this.caretPosition;
    const relative = this.caretPosition - this.doc.translationStart;
    const locale = this.targetLocale;
    const probe = direction < 0 ? Math.max(0, relative - 1) : relative;
    const boundary = getWordBoundary(locale, this.doc.translation, probe, direction > 0);
    const next = this.doc.translationStart + boundary;
    if (extendSelection) {
      this.caretPosition = next;
      this.selectionFocus = next;
      this.notifyWordAtCaret();
    } else {
      this.setCaretPosition(next, direction < 0 ? "before" : "after");
    }
    return this.caretPosition;
  }

  deleteToken(direction: -1 | 1): boolean {
    if (this.getSelectionStart() !== this.getSelectionEnd()) return this.deleteSelectionAtomic();
    const origin = this.caretPosition;
    const boundary = this.moveByToken(direction, true);
    if (boundary === origin) return false;
    return this.deleteSelectionAtomic();
  }

  /**
   * Swing's paste/cut hooks forcibly trim a roaming selection back to the
   * active translation before mutating the document.
   */
  clampSelectionToTranslation(force = true): void {
    if (!force && !this.lockCursorToInputArea) return;
    if (!this.doc.editMode) return;
    const clamp = (position: number) =>
      Math.max(this.doc.translationStart, Math.min(position, this.doc.translationEnd));
    this.selectionAnchor = clamp(this.selectionAnchor);
    this.selectionFocus = clamp(this.selectionFocus);
    this.caretPosition = this.selectionFocus;
    this.notifyWordAtCaret();
  }

  pasteText(text: string): boolean {
    this.clampSelectionToTranslation();
    return this.replaceSelection(text);
  }

  cutSelection(): string | null {
    this.clampSelectionToTranslation();
    if (this.getSelectionStart() === this.getSelectionEnd()) return null;
    const selected = this.getSelectedText();
    return this.deleteSelectionAtomic() ? selected : null;
  }

  selectTagAt(position: number): boolean {
    if (!this.isInActiveTranslation(position)) return false;
    const relative = position - this.doc.translationStart;
    const tag = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;
    for (const match of this.doc.translation.matchAll(tag)) {
      const start = match.index ?? 0;
      const end = start + match[0].length;
      if (relative >= start && relative < end) {
        this.setSelection(this.doc.translationStart + start, this.doc.translationStart + end);
        return true;
      }
    }
    return false;
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
    this.notifyWordAtCaret();
  }

  setTargetLocale(locale: string): void {
    this.targetLocale = locale || "und";
    this.notifyWordAtCaret();
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

  focus(): void {
    if (this.focused) return;
    this.focused = true;
    for (const listener of this.focusListeners) listener(true);
  }

  blur(): void {
    if (!this.focused) return;
    if (this.composition) this.commitComposition();
    this.focused = false;
    for (const listener of this.focusListeners) listener(false);
  }

  hasFocus(): boolean {
    return this.focused;
  }

  onFocusChanged(listener: (focused: boolean) => void): () => void {
    this.focusListeners.add(listener);
    return () => this.focusListeners.delete(listener);
  }

  private normalizeCaretPosition(position: number, bias: "before" | "after"): number {
    let next = Math.max(0, Math.min(position, this.doc.fullText.length));
    if (this.lockCursorToInputArea && this.doc.editMode) {
      next = Math.max(this.doc.translationStart, Math.min(next, this.doc.translationEnd));
    }
    const relative = next - this.doc.translationStart;
    if (relative >= 0 && relative <= this.doc.translation.length) {
      next = this.doc.translationStart + snapCaret(this.doc.translation, relative, bias);
    }
    return next;
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
    const locale = inTarget ? this.targetLocale : this.sourceLocale;
    if (word === this.currentWord && locale === this.currentWordLocale) return;
    this.currentWord = word;
    this.currentWordLocale = locale;
    for (const listener of this.wordListeners) listener(word, locale);
  }
}
