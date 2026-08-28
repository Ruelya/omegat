// SPDX-License-Identifier: GPL-3.0-or-later

import {
  commitAndDeactivate,
  type Document3State,
} from "./Document3";
import { EditorTextArea3, type ProtectedRange } from "./EditorTextArea3";
import { buildActiveDocument } from "./SegmentBuilder";

export type EditorCaretPosition = {
  position?: number;
  selectionStart?: number;
  selectionEnd?: number;
};

export type EditorDocumentPresentation = {
  document: Document3State;
  protectedRanges?: readonly ProtectedRange[];
};

/**
 * Owns the active `Document3` and its activation/deactivation boundary.
 *
 * Java rebuilds and marks the segment before exposing it through
 * `EditorTextArea3`; the presentation callback preserves that ordering while
 * keeping Marker policy outside the document lifecycle.
 */
export class EditorDocumentLifecycle {
  readonly textArea = new EditorTextArea3();
  private current: Document3State | null = null;

  get document(): Document3State | null {
    return this.current;
  }

  setCurrent(document: Document3State | null): void {
    this.current = document;
  }

  activate(
    entryNumber: number,
    source: string,
    translation: string,
    position: EditorCaretPosition = { position: 0 },
    present: (document: Document3State) => EditorDocumentPresentation =
      (document) => ({ document }),
  ): Document3State {
    this.current = buildActiveDocument(entryNumber, source, translation);
    this.applyPresentation(present(this.current));
    this.setCaretPosition(position);
    return this.current;
  }

  /**
   * Pull direct textarea and IME edits into the active document before a
   * commit or navigation can replace it.
   */
  adoptLiveDocument(): Document3State | null {
    if (!this.current) return null;
    const live = this.textArea.getOmDocument();
    if (live.source !== this.current.source) return this.current;
    if (this.textArea.isComposing()) this.textArea.commitComposition();
    this.current = this.textArea.getOmDocument();
    return this.current;
  }

  /** Stop edit mode after first adopting every live edit. */
  commit(deactivate: boolean): Document3State | null {
    this.adoptLiveDocument();
    if (deactivate && this.current?.editMode) {
      this.current = commitAndDeactivate(this.current);
    }
    return this.current;
  }

  applyPresentation(
    presentation: EditorDocumentPresentation,
    preserveSelection = false,
  ): void {
    this.current = presentation.document;
    this.textArea.setDocument(this.current, preserveSelection);
    this.textArea.setProtectedRanges(presentation.protectedRanges ?? []);
  }

  clear(): void {
    this.current = null;
    this.textArea.setDocument(buildActiveDocument(0, "", ""));
    this.textArea.setProtectedRanges([]);
  }

  getPositionInEntryTranslation(position: number): number {
    const document = this.current;
    if (!document?.editMode) return -1;
    return Math.max(
      0,
      Math.min(position, document.translationEnd) - document.translationStart,
    );
  }

  getCurrentPositionInEntryTranslation(): number {
    return this.getPositionInEntryTranslation(this.textArea.getCaretPosition());
  }

  getCaretPosition(): EditorCaretPosition {
    const document = this.current;
    if (!document?.editMode) return { position: -1 };
    const start = this.getPositionInEntryTranslation(this.textArea.getSelectionStart());
    const end = this.getPositionInEntryTranslation(this.textArea.getSelectionEnd());
    if (start === end) {
      return {
        position: this.getPositionInEntryTranslation(this.textArea.getCaretPosition()),
      };
    }
    return { selectionStart: start, selectionEnd: end };
  }

  setCaretPosition(position: EditorCaretPosition): void {
    const document = this.current;
    if (!document?.editMode) return;
    if (position.position !== undefined) {
      this.textArea.setCaretPosition(document.translationStart + position.position);
    } else if (
      position.selectionStart !== undefined
      && position.selectionEnd !== undefined
    ) {
      this.textArea.setSelection(
        document.translationStart + position.selectionStart,
        document.translationStart + position.selectionEnd,
      );
    }
    this.textArea.clampSelectionToTranslation();
  }

  getSelectedText(): string {
    if (!this.current?.editMode) return "";
    if (this.textArea.getOmDocument() !== this.current) {
      this.applyPresentation({ document: this.current }, true);
    }
    return this.textArea.getSelectedText();
  }
}
