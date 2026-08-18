/** Java `org.omegat.gui.editor.EditorController` — IEditor implementation host. */
import { createDocument3, type Document3State } from "./Document3";
import { IEditor } from "./IEditor";
import { MarkerController } from "./MarkerController";
import { buildActiveDocument } from "./SegmentBuilder";
import { TranslationUndoManager } from "./TranslationUndoManager";

export type LoadedEntry = {
  file: string;
  source: string;
  translation: string;
  id?: string;
};

export class EditorController {
  readonly editor = IEditor;
  readonly markers = new MarkerController();
  readonly undo = new TranslationUndoManager();
  displayedFileIndex = 0;
  document: Document3State | null = null;
  currentFile: string | null = null;
  currentEntryNumber = 0;
  entries: LoadedEntry[] = [];

  getCurrentTranslation() {
    return this.editor.getCurrentTranslation();
  }
  replaceEditText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.replaceEditText(text);
  }
  insertText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.insertText(text);
  }
  commitAndDeactivate() {
    return this.editor.commitAndDeactivate();
  }

  isOrientationAllLtr(): boolean {
    return this.editor.isOrientationAllLtr();
  }

  getCurrentFile(): string | null {
    return this.currentFile;
  }

  getCurrentEntryNumber(): number {
    return this.currentEntryNumber;
  }

  getOmDocument(): Document3State | null {
    return this.document;
  }

  loadEmptyProject(): void {
    this.entries = [];
    this.document = null;
    this.currentFile = null;
    this.currentEntryNumber = 0;
    this.displayedFileIndex = 0;
  }

  /** Java `EditorControllerTest#testEditorControllerLoadSimpleProject` fixture. */
  loadSimpleProject(): void {
    this.entries = [
      { file: "source.txt", source: "XXX", translation: "" },
      {
        file: "website/download.html",
        source: "Other",
        translation: "",
        id: "id",
      },
    ];
    this.displayedFileIndex = 0;
    this.activateEntry(0);
  }

  activateEntry(index: number): void {
    const e = this.entries[index];
    if (!e) {
      this.document = null;
      this.currentFile = null;
      this.currentEntryNumber = 0;
      return;
    }
    this.currentFile = e.file;
    this.currentEntryNumber = index + 1;
    this.document = buildActiveDocument(this.currentEntryNumber, e.source, e.translation);
  }
}

export function createEditorController(): EditorController {
  const c = new EditorController();
  return c;
}
