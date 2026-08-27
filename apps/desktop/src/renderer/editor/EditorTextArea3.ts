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

export type ProtectedRange = {
  start: number;
  end: number;
  tooltip?: string;
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
  private currentWordLocale = "und";
  private focused = false;
  private composition: CompositionSession | null = null;
  private mouseSelectionActive = false;
  private protectedRanges: ProtectedRange[] = [];
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
    this.mouseSelectionActive = false;
    this.protectedRanges = [];
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

  /**
   * Install Java `ProtectedPart` intervals resolved by MarkerController.
   * Positions are UTF-16 offsets relative to the active translation.
   */
  setProtectedRanges(ranges: readonly ProtectedRange[]): void {
    this.protectedRanges = ranges
      .filter(({ start, end }) =>
        Number.isInteger(start)
        && Number.isInteger(end)
        && start >= 0
        && end > start
        && end <= this.doc.translation.length
      )
      .map((range) => ({ ...range }))
      .sort((a, b) => a.start - b.start || a.end - b.end);
  }

  getProtectedRanges(): ProtectedRange[] {
    return this.protectedRanges.map((range) => ({ ...range }));
  }

  getProtectedTooltipAt(offset: number): string | null {
    const tips = this.protectedRanges
      .filter((range) => offset >= range.start && offset <= range.end)
      .flatMap((range) => range.tooltip ? [range.tooltip] : []);
    return tips.length ? tips.join("<br>") : null;
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

  /**
   * Start a native mouse drag at a renderer-relative UTF-16 offset. The
   * renderer owns pointer capture, while this model owns the directional
   * selection over the active Document3 translation.
   */
  beginMouseSelection(
    offset: number,
    bias: "before" | "after" = "after",
    extendSelection = false,
  ): number {
    this.setCaretFromRenderedOffset(offset, bias, extendSelection);
    this.mouseSelectionActive = true;
    return this.caretPosition;
  }

  updateMouseSelection(offset: number, bias: "before" | "after" = "after"): boolean {
    if (!this.mouseSelectionActive) return false;
    this.setCaretFromRenderedOffset(offset, bias, true);
    return true;
  }

  endMouseSelection(
    offset?: number,
    bias: "before" | "after" = "after",
  ): boolean {
    if (!this.mouseSelectionActive) return false;
    if (offset !== undefined) this.setCaretFromRenderedOffset(offset, bias, true);
    this.mouseSelectionActive = false;
    return true;
  }

  isMouseSelecting(): boolean {
    return this.mouseSelectionActive;
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
    const relative = this.expandProtectedSelection(
      this.getSelectionStart() - this.doc.translationStart,
      this.getSelectionEnd() - this.doc.translationStart,
    );
    const start = this.doc.translationStart + relative.start;
    const end = this.doc.translationStart + relative.end;
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
    const relative = this.expandProtectedSelection(
      this.getSelectionStart() - this.doc.translationStart,
      this.getSelectionEnd() - this.doc.translationStart,
    );
    const start = this.doc.translationStart + relative.start;
    const end = this.doc.translationStart + relative.end;
    this.selectionAnchor = start;
    this.selectionFocus = end;
    this.caretPosition = end;
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
    const protectedPart = this.protectedRanges.find(
      ({ start, end }) => start < relative && relative <= end,
    );
    if (protectedPart) {
      this.doc = applyDocumentEdit(
        this.doc,
        this.doc.translationStart + protectedPart.start,
        protectedPart.end - protectedPart.start,
        "",
      );
      this.setCaretPosition(this.doc.translationStart + protectedPart.start);
      return true;
    }
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
    const protectedPart = this.protectedRanges.find(
      ({ start, end }) => start <= relative && relative < end,
    );
    if (protectedPart) {
      this.doc = applyDocumentEdit(
        this.doc,
        this.doc.translationStart + protectedPart.start,
        protectedPart.end - protectedPart.start,
        "",
      );
      this.setCaretPosition(this.doc.translationStart + protectedPart.start);
      return true;
    }
    const next = deleteForwardAtomic(this.doc.translation, relative);
    if (next.text === this.doc.translation) return false;
    const removed = this.doc.translation.length - next.text.length;
    this.doc = applyDocumentEdit(this.doc, this.doc.translationStart + next.pos, removed, "");
    this.setCaretPosition(this.doc.translationStart + next.pos);
    return true;
  }

  deleteSelectionAtomic(): boolean {
    const range = this.expandProtectedSelection(
      this.getSelectionStart() - this.doc.translationStart,
      this.getSelectionEnd() - this.doc.translationStart,
    );
    const next = deleteRangeAtomic(this.doc.translation, range.start, range.end);
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
    const protectedPart = this.protectedRanges.find(({ start, end }) =>
      direction < 0
        ? start < relative && relative <= end
        : start <= relative && relative < end
    );
    const next = this.doc.translationStart + (
      protectedPart
        ? direction < 0 ? protectedPart.start : protectedPart.end
        : moveCaret(this.doc.translation, relative, direction)
    );
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

  selectProtectedPartAt(position: number): boolean {
    if (!this.isInActiveTranslation(position)) return false;
    const relative = position - this.doc.translationStart;
    const range = this.protectedRanges.find(
      ({ start, end }) => relative >= start && relative < end,
    );
    if (range) {
      const start = this.expandDirectionStart(range.start);
      const end = this.expandDirectionEnd(range.end);
      this.setSelection(this.doc.translationStart + start, this.doc.translationStart + end);
      return true;
    }
    return false;
  }

  selectTagAt(position: number): boolean {
    if (this.selectProtectedPartAt(position)) return true;
    if (!this.isInActiveTranslation(position)) return false;
    const relative = position - this.doc.translationStart;
    const tag = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;
    for (const match of this.doc.translation.matchAll(tag)) {
      const start = match.index ?? 0;
      const end = start + match[0].length;
      if (relative >= start && relative < end) {
        this.setSelection(
          this.doc.translationStart + this.expandDirectionStart(start),
          this.doc.translationStart + this.expandDirectionEnd(end),
        );
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
    if (this.composition) this.commitComposition();
    if (!this.focused) return;
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
      const protectedPart = this.protectedRanges.find(
        ({ start, end }) => start < relative && relative < end,
      );
      const protectedOffset = protectedPart
        ? bias === "before" ? protectedPart.start : protectedPart.end
        : relative;
      next = this.doc.translationStart + snapCaret(
        this.doc.translation,
        protectedOffset,
        bias,
      );
    }
    return next;
  }

  private expandProtectedSelection(start: number, end: number): { start: number; end: number } {
    let expandedStart = Math.max(0, Math.min(start, end));
    let expandedEnd = Math.min(this.doc.translation.length, Math.max(start, end));
    for (const range of this.protectedRanges) {
      if (expandedStart < range.end && expandedEnd > range.start) {
        expandedStart = Math.min(expandedStart, range.start);
        expandedEnd = Math.max(expandedEnd, range.end);
      }
    }
    return { start: expandedStart, end: expandedEnd };
  }

  private expandDirectionStart(start: number): number {
    for (let count = 0; count < 2 && start > 0; count += 1) {
      if (!isDirectionChar(this.doc.translation[start - 1]!)) break;
      start -= 1;
    }
    return start;
  }

  private expandDirectionEnd(end: number): number {
    for (let count = 0; count < 2 && end < this.doc.translation.length; count += 1) {
      if (!isDirectionChar(this.doc.translation[end]!)) break;
      end += 1;
    }
    return end;
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

function isDirectionChar(char: string): boolean {
  return char === "\u200e"
    || char === "\u200f"
    || char === "\u202a"
    || char === "\u202b"
    || char === "\u202c";
}
