/** Java `org.omegat.gui.editor.EditorController` — IEditor implementation host. */
import {
  commitAndDeactivate as commitDocument,
  replaceEditText as replaceDocumentText,
  type Document3State,
} from "./Document3";
import { IEditor } from "./IEditor";
import { makeFilter, type IEditorFilter } from "./IEditorFilter";
import { MarkerController, type MarkerSnapshot } from "./MarkerController";
import { buildActiveDocument } from "./SegmentBuilder";
import { SegmentHistory } from "./SegmentHistory";
import { EditorTextArea3 } from "./EditorTextArea3";
import { TranslationUndoManager } from "./TranslationUndoManager";
import type { MarkerInput, ProtectedPart } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";

export type LoadedEntry = {
  file: string;
  source: string;
  translation: string;
  id?: string;
  note?: string;
  unique?: boolean;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  protectedParts?: ProtectedPart[];
};

export type LoadedPageEntry = {
  key: string;
  index: number;
  entryNumber: number;
  file: string;
  source: string;
  translation: string;
  active: boolean;
  marks: Mark[];
};

export class EditorController {
  readonly editor = IEditor;
  readonly textArea = new EditorTextArea3();
  readonly markers = new MarkerController();
  readonly undo = new TranslationUndoManager();
  readonly history = new SegmentHistory();
  displayedFileIndex = 0;
  previousDisplayedFileIndex = 0;
  displayedEntryIndex = -1;
  firstLoaded = -1;
  lastLoaded = -1;
  document: Document3State | null = null;
  currentFile: string | null = null;
  currentEntryNumber = 0;
  entries: LoadedEntry[] = [];
  sourceLangIsRTL = false;
  targetLangIsRTL = false;
  markerSnapshot: MarkerSnapshot | null = null;
  private entriesFilter: IEditorFilter = makeFilter("none");
  private visibleEntryIndices: number[] = [];
  private pageRadius = 25;

  getCurrentTranslation(): string {
    return this.document?.translation ?? this.editor.getCurrentTranslation();
  }

  replaceEditText(text: string): void {
    this.undo.remember(this.getCurrentTranslation());
    if (!this.document) {
      this.editor.replaceEditText(text);
      return;
    }
    this.document = replaceDocumentText(this.document, text);
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
  }

  insertText(text: string): void {
    this.undo.remember(this.getCurrentTranslation());
    if (!this.document) {
      this.editor.insertText(text);
      return;
    }
    this.textArea.setDocument(this.document);
    this.textArea.setCaretPosition(this.document.translationEnd);
    this.textArea.insertText(text);
    this.document = this.textArea.getOmDocument();
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
  }

  async commitAndDeactivate(): Promise<void> {
    if (!this.document) {
      await this.editor.commitAndDeactivate();
      return;
    }
    this.syncActiveEntry();
    this.document = commitDocument(this.document);
    this.textArea.setDocument(this.document);
  }

  isOrientationAllLtr(): boolean {
    return !this.sourceLangIsRTL && !this.targetLangIsRTL;
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

  getCurrentEntry(): LoadedEntry | null {
    return this.entries[this.displayedEntryIndex] ?? null;
  }

  loadProject(entries: LoadedEntry[], preferredEntryNumber = 1): void {
    this.entries = entries.map((entry) => ({ ...entry }));
    this.markers.invalidate();
    this.markerSnapshot = null;
    this.rebuildVisibleEntries();
    this.history.back = [];
    this.history.forward = [];
    if (this.visibleEntryIndices.length === 0) {
      this.loadEmptyProject();
      return;
    }
    const requested = Math.max(0, Math.min(preferredEntryNumber - 1, this.entries.length - 1));
    const initial = this.visibleEntryIndices.includes(requested) ? requested : this.visibleEntryIndices[0]!;
    this.activateEntry(initial);
  }

  /**
   * Bind the renderer's immutable project snapshot to the same paging model
   * used by the headless controller. This deliberately does not touch
   * navigation or undo history: Zustand owns those while React is mounted.
   */
  synchronizeRendererProject(
    entries: readonly LoadedEntry[],
    activeIndex: number,
    document: Document3State,
  ): LoadedPageEntry[] {
    this.entries = entries.map((entry, index) => ({
      ...entry,
      translation: index === activeIndex ? document.translation : entry.translation,
    }));
    this.rebuildVisibleEntries();
    const safeIndex = Math.max(0, Math.min(activeIndex, this.entries.length - 1));
    const active = this.entries[safeIndex];
    if (!active || !this.visibleEntryIndices.includes(safeIndex)) {
      this.document = null;
      this.currentFile = null;
      this.currentEntryNumber = 0;
      this.displayedEntryIndex = -1;
      this.firstLoaded = -1;
      this.lastLoaded = -1;
      this.markerSnapshot = null;
      return [];
    }
    const files = [...new Set(this.entries.map((entry) => entry.file))];
    this.previousDisplayedFileIndex = this.displayedFileIndex;
    this.displayedFileIndex = Math.max(0, files.indexOf(active.file));
    this.displayedEntryIndex = safeIndex;
    this.currentFile = active.file;
    this.currentEntryNumber = safeIndex + 1;
    this.document = document;
    this.markerSnapshot = this.markers.processEntry(
      this.entryKey(safeIndex, active),
      this.markerInput(active, true),
    );
    this.textArea.setDocument(document, true);
    this.loadWindowAround(safeIndex);
    return this.getLoadedPage();
  }

  loadEmptyProject(): void {
    this.entries = [];
    this.visibleEntryIndices = [];
    this.document = null;
    this.currentFile = null;
    this.currentEntryNumber = 0;
    this.displayedFileIndex = 0;
    this.previousDisplayedFileIndex = 0;
    this.displayedEntryIndex = -1;
    this.firstLoaded = -1;
    this.lastLoaded = -1;
    this.history.back = [];
    this.history.forward = [];
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    this.markers.invalidate();
    this.markerSnapshot = null;
  }

  /** Java `EditorControllerTest#testEditorControllerLoadSimpleProject` fixture. */
  loadSimpleProject(): void {
    this.loadProject([
      { file: "source.txt", source: "XXX", translation: "" },
      {
        file: "website/download.html",
        source: "Other",
        translation: "",
        id: "id",
      },
    ]);
  }

  activateEntry(index: number, recordHistory = true): void {
    const e = this.entries[index];
    if (!e || !this.entriesFilter.allowed(e)) {
      this.document = null;
      this.currentFile = null;
      this.currentEntryNumber = 0;
      this.displayedEntryIndex = -1;
      return;
    }
    this.syncActiveEntry();
    const files = [...new Set(this.entries.map((entry) => entry.file))];
    this.previousDisplayedFileIndex = this.displayedFileIndex;
    this.displayedFileIndex = Math.max(0, files.indexOf(e.file));
    this.displayedEntryIndex = index;
    this.currentFile = e.file;
    this.currentEntryNumber = index + 1;
    this.document = buildActiveDocument(this.currentEntryNumber, e.source, e.translation);
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    this.loadWindowAround(index);
    if (recordHistory && this.history.back.at(-1) !== this.currentEntryNumber) {
      this.history.visit(this.currentEntryNumber);
    }
  }

  gotoEntry(entryNumber: number): boolean {
    const index = entryNumber - 1;
    if (!this.visibleEntryIndices.includes(index)) return false;
    if (index === this.displayedEntryIndex) return false;
    this.activateEntry(index);
    return true;
  }

  gotoFile(file: string): boolean {
    const index = this.visibleEntryIndices.find((candidate) => this.entries[candidate]?.file === file);
    return index === undefined ? false : this.gotoEntry(index + 1);
  }

  nextEntry(): boolean {
    return this.moveVisible(1);
  }

  prevEntry(): boolean {
    return this.moveVisible(-1);
  }

  gotoHistoryBack(): boolean {
    const entryNumber = this.history.goBack();
    if (entryNumber === undefined) return false;
    this.activateEntry(entryNumber - 1, false);
    return true;
  }

  gotoHistoryForward(): boolean {
    const entryNumber = this.history.goForward();
    if (entryNumber === undefined) return false;
    this.activateEntry(entryNumber - 1, false);
    return true;
  }

  undoEdit(): string {
    const next = this.undo.undo(this.getCurrentTranslation());
    if (next !== this.getCurrentTranslation()) this.replaceWithoutHistory(next);
    return next;
  }

  redoEdit(): string {
    const next = this.undo.redo(this.getCurrentTranslation());
    if (next !== this.getCurrentTranslation()) this.replaceWithoutHistory(next);
    return next;
  }

  setFilter(filter: IEditorFilter): void {
    this.entriesFilter = filter;
    this.rebuildVisibleEntries();
    if (this.displayedEntryIndex >= 0 && !this.visibleEntryIndices.includes(this.displayedEntryIndex)) {
      const next = this.visibleEntryIndices[0];
      if (next === undefined) {
        this.document = null;
        this.currentFile = null;
        this.currentEntryNumber = 0;
        this.displayedEntryIndex = -1;
      } else {
        this.activateEntry(next);
      }
    } else if (this.displayedEntryIndex >= 0) {
      this.loadWindowAround(this.displayedEntryIndex);
    }
  }

  removeFilter(): void {
    this.setFilter(makeFilter("none"));
  }

  getFilter(): IEditorFilter {
    return this.entriesFilter;
  }

  getLoadedRange(): { first: number; last: number } {
    return { first: this.firstLoaded, last: this.lastLoaded };
  }

  getLoadedPage(): LoadedPageEntry[] {
    if (this.firstLoaded < 0 || this.lastLoaded < this.firstLoaded) return [];
    const first = this.visibleEntryIndices.indexOf(this.firstLoaded);
    const last = this.visibleEntryIndices.indexOf(this.lastLoaded);
    if (first < 0 || last < first) return [];
    return this.visibleEntryIndices
      .slice(first, last + 1)
      .map((index) => {
        const entry = this.entries[index]!;
        const active = index === this.displayedEntryIndex;
        const snapshot = active && this.markerSnapshot
          ? this.markerSnapshot
          : this.markers.processEntry(this.entryKey(index, entry), this.markerInput(entry, active));
        return {
          key: this.entryKey(index, entry),
          index,
          entryNumber: index + 1,
          file: entry.file,
          source: entry.source,
          translation: entry.translation,
          active,
          marks: snapshot.marks,
        };
      });
  }

  setPageRadius(radius: number): void {
    this.pageRadius = Math.max(0, Math.floor(radius));
    if (this.displayedEntryIndex >= 0) this.loadWindowAround(this.displayedEntryIndex);
  }

  loadUp(count: number): number {
    const first = this.visibleEntryIndices.indexOf(this.firstLoaded);
    if (first <= 0) return 0;
    const next = Math.max(0, first - Math.max(0, Math.floor(count)));
    this.firstLoaded = this.visibleEntryIndices[next]!;
    return first - next;
  }

  loadDown(count: number): number {
    const last = this.visibleEntryIndices.indexOf(this.lastLoaded);
    if (last < 0 || last >= this.visibleEntryIndices.length - 1) return 0;
    const next = Math.min(
      this.visibleEntryIndices.length - 1,
      last + Math.max(0, Math.floor(count)),
    );
    this.lastLoaded = this.visibleEntryIndices[next]!;
    return next - last;
  }

  hasMoreBefore(): boolean {
    return this.visibleEntryIndices.indexOf(this.firstLoaded) > 0;
  }

  hasMoreAfter(): boolean {
    const last = this.visibleEntryIndices.indexOf(this.lastLoaded);
    return last >= 0 && last < this.visibleEntryIndices.length - 1;
  }

  private moveVisible(delta: -1 | 1): boolean {
    const visiblePosition = this.visibleEntryIndices.indexOf(this.displayedEntryIndex);
    if (visiblePosition < 0) return false;
    const target = this.visibleEntryIndices[visiblePosition + delta];
    if (target === undefined) return false;
    this.activateEntry(target);
    return true;
  }

  private rebuildVisibleEntries(): void {
    this.visibleEntryIndices = this.entries.flatMap((entry, index) =>
      this.entriesFilter.allowed(entry) ? [index] : [],
    );
  }

  private loadWindowAround(index: number, radius = this.pageRadius): void {
    const visiblePosition = this.visibleEntryIndices.indexOf(index);
    if (visiblePosition < 0) {
      this.firstLoaded = -1;
      this.lastLoaded = -1;
      return;
    }
    const first = Math.max(0, visiblePosition - radius);
    const last = Math.min(this.visibleEntryIndices.length - 1, visiblePosition + radius);
    this.firstLoaded = this.visibleEntryIndices[first]!;
    this.lastLoaded = this.visibleEntryIndices[last]!;
  }

  private replaceWithoutHistory(text: string): void {
    if (!this.document) {
      this.editor.replaceEditText(text);
      return;
    }
    this.document = replaceDocumentText(this.document, text);
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
  }

  private syncActiveEntry(): void {
    if (!this.document || this.displayedEntryIndex < 0) return;
    const entry = this.entries[this.displayedEntryIndex];
    if (entry) entry.translation = this.document.translation;
  }

  private markerInput(entry: LoadedEntry, active: boolean): MarkerInput {
    return {
      sourceText: entry.source,
      translationText: entry.translation,
      isActive: active,
      isAlt: entry.isAlt,
      fromAuto: entry.fromAuto,
      fromMt: entry.fromMt,
      protectedParts: entry.protectedParts,
    };
  }

  private entryKey(index: number, entry: LoadedEntry): string {
    return `${index}:${entry.file}:${entry.id ?? ""}`;
  }

  private refreshCurrentMarkers(): void {
    if (!this.document || this.displayedEntryIndex < 0) {
      this.markerSnapshot = null;
      return;
    }
    const entry = this.entries[this.displayedEntryIndex];
    if (!entry) {
      this.markerSnapshot = null;
      return;
    }
    const marked = this.markers.applyToDocument(
      this.entryKey(this.displayedEntryIndex, entry),
      this.document,
      this.markerInput(entry, true),
    );
    this.document = marked.document;
    this.markerSnapshot = marked.snapshot;
  }

  /** Drop per-project document state (EditorProjectReloadLeakTest). */
  closeProject(): void {
    this.loadEmptyProject();
  }
}

export function createEditorController(): EditorController {
  const c = new EditorController();
  return c;
}
