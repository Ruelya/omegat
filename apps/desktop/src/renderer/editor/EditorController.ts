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
import type { EntryKeyDto } from "../lib/types";

export type LoadedEntry = {
  key?: EntryKeyDto;
  file: string;
  source: string;
  translation: string;
  id?: string;
  note?: string;
  unique?: boolean;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  translated?: boolean;
  linked?: "xICE" | "x100PC" | "xAUTO" | "xENFORCED";
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

export type ScrollAnchorCandidate = {
  key: string;
  top: number;
  bottom: number;
};

export type EditorScrollAnchor = {
  key: string;
  offset: number;
};

export type EditorCaretPosition = {
  position?: number;
  selectionStart?: number;
  selectionEnd?: number;
};

type EditorUndoState = {
  translation: string;
  caret: EditorCaretPosition;
};

/**
 * Find the next matching entry with Java's project-wide wraparound behavior.
 * The current entry is considered last, after every other entry was checked.
 */
export function findCyclicEntryIndex<T>(
  entries: readonly T[],
  currentIndex: number,
  direction: -1 | 1,
  allowed: (entry: T, index: number) => boolean = () => true,
  matches: (entry: T, index: number) => boolean = () => true,
): number | null {
  if (entries.length === 0) return null;
  const origin = Math.max(0, Math.min(currentIndex, entries.length - 1));
  for (let distance = 1; distance <= entries.length; distance += 1) {
    const index = (origin + direction * distance + entries.length * 2) % entries.length;
    const entry = entries[index]!;
    if (allowed(entry, index) && matches(entry, index)) return index;
  }
  return null;
}

export class EditorController {
  readonly editor = IEditor;
  readonly textArea = new EditorTextArea3();
  readonly markers = new MarkerController();
  readonly undo = new TranslationUndoManager<EditorUndoState>();
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
    if (!this.document) {
      this.editor.replaceEditText(text);
      return;
    }
    this.adoptLiveDocument();
    if (text === this.getCurrentTranslation()) return;
    this.undo.remember(this.currentUndoState());
    this.document = replaceDocumentText(this.document, text);
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
  }

  insertText(text: string): void {
    if (!this.document) {
      this.editor.insertText(text);
      return;
    }
    this.adoptLiveDocument();
    this.textArea.setDocument(this.document, true);
    this.textArea.clampSelectionToTranslation();
    const before = this.currentUndoState();
    if (!this.textArea.replaceSelection(text)) return;
    this.undo.remember(before);
    this.document = this.textArea.getOmDocument();
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document, true);
  }

  async commitAndDeactivate(): Promise<void> {
    if (!this.document) {
      await this.editor.commitAndDeactivate();
      return;
    }
    this.commitCurrentDocument(true);
  }

  /**
   * Commit without changing entries and restore the relative caret, matching
   * Java's deactivate/activate cycle.
   */
  commitAndLeave(): void {
    if (!this.document || this.displayedEntryIndex < 0) return;
    this.adoptLiveDocument();
    const position = this.getCurrentPositionInEntryTranslation();
    const index = this.displayedEntryIndex;
    this.commitCurrentDocument(true);
    this.openEntry(index, false, { position });
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

  getPositionInEntryTranslation(position: number): number {
    const doc = this.document;
    if (!doc?.editMode) return -1;
    return Math.max(
      0,
      Math.min(position, doc.translationEnd) - doc.translationStart,
    );
  }

  getCurrentPositionInEntryTranslation(): number {
    return this.getPositionInEntryTranslation(this.textArea.getCaretPosition());
  }

  getCurrentPositionInEntryTranslationInEditor(): EditorCaretPosition {
    const doc = this.document;
    if (!doc?.editMode) return { position: -1 };
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
    const doc = this.document;
    if (!doc?.editMode) return;
    this.textArea.setDocument(doc, true);
    if (position.position !== undefined) {
      this.textArea.setCaretPosition(doc.translationStart + position.position);
    } else if (
      position.selectionStart !== undefined
      && position.selectionEnd !== undefined
    ) {
      this.textArea.setSelection(
        doc.translationStart + position.selectionStart,
        doc.translationStart + position.selectionEnd,
      );
    }
    this.textArea.clampSelectionToTranslation();
  }

  getSelectedText(): string {
    if (!this.document?.editMode) return "";
    if (this.textArea.getOmDocument() !== this.document) {
      this.textArea.setDocument(this.document, true);
    }
    return this.textArea.getSelectedText();
  }

  getCurrentEntry(): LoadedEntry | null {
    return this.entries[this.displayedEntryIndex] ?? null;
  }

  setCurrentTranslationVariant(defaultTranslation: boolean): void {
    const entry = this.getCurrentEntry();
    if (!entry) return;
    entry.isAlt = !defaultTranslation;
    this.refreshCurrentMarkers();
  }

  registerPluginMarker(name: string, marker: import("./mark/IMarker").MarkerProvider): void {
    this.markers.registerPluginMarker(name, marker);
    this.refreshCurrentMarkers();
  }

  unregisterPluginMarker(name: string): boolean {
    const removed = this.markers.unregisterPluginMarker(name);
    if (removed) this.refreshCurrentMarkers();
    return removed;
  }

  remarkOneMarker(name: string): void {
    this.markers.remarkOneMarker(name);
    this.refreshCurrentMarkers();
  }

  loadProject(entries: LoadedEntry[], preferredEntryNumber = 1): void {
    this.commitCurrentDocument(true);
    this.document = null;
    this.displayedEntryIndex = -1;
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
    this.commitCurrentDocument(true);
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

  activateEntry(
    index: number,
    recordHistory = true,
    position: EditorCaretPosition = { position: 0 },
  ): void {
    const e = this.entries[index];
    if (!e || !this.entriesFilter.allowed(e)) return;
    this.commitCurrentDocument(true);
    this.openEntry(index, recordHistory, position);
  }

  private openEntry(
    index: number,
    recordHistory: boolean,
    position: EditorCaretPosition,
  ): void {
    const e = this.entries[index];
    if (!e || !this.entriesFilter.allowed(e)) return;
    const files = [...new Set(this.entries.map((entry) => entry.file))];
    this.previousDisplayedFileIndex = this.displayedFileIndex;
    this.displayedFileIndex = Math.max(0, files.indexOf(e.file));
    this.displayedEntryIndex = index;
    this.currentFile = e.file;
    this.currentEntryNumber = index + 1;
    this.document = buildActiveDocument(this.currentEntryNumber, e.source, e.translation);
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
    this.setCaretPosition(position);
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
    const changed = index !== this.displayedEntryIndex;
    this.activateEntry(index);
    return changed;
  }

  gotoFile(file: string | number): boolean {
    const files = [...new Set(this.entries.map((entry) => entry.file))];
    const fileName = typeof file === "number" ? files[file] : file;
    if (fileName === undefined) {
      if (typeof file === "number") throw new RangeError("file index out of bounds");
      return false;
    }
    const index = this.visibleEntryIndices.find((candidate) => this.entries[candidate]?.file === fileName);
    if (index === undefined) return false;
    const changed = index !== this.displayedEntryIndex;
    this.activateEntry(index);
    return changed;
  }

  nextEntry(): boolean {
    return this.moveVisible(1);
  }

  prevEntry(): boolean {
    return this.moveVisible(-1);
  }

  nextUntranslatedEntry(): boolean {
    return this.moveMatching(1, (entry) => !(entry.translated ?? entry.translation.length > 0));
  }

  nextTranslatedEntry(): boolean {
    return this.moveMatching(1, (entry) => entry.translated ?? entry.translation.length > 0);
  }

  nextUniqueEntry(): boolean {
    return this.moveMatching(1, (entry) => entry.unique !== false);
  }

  nextEntryWithNote(): boolean {
    return this.moveMatching(1, (entry) => Boolean(entry.note));
  }

  prevEntryWithNote(): boolean {
    return this.moveMatching(-1, (entry) => Boolean(entry.note));
  }

  nextXAutoEntry(): boolean {
    return this.moveMatching(1, (entry) => entry.linked === "xAUTO" || entry.fromAuto === true);
  }

  prevXAutoEntry(): boolean {
    return this.moveMatching(-1, (entry) => entry.linked === "xAUTO" || entry.fromAuto === true);
  }

  nextXEnforcedEntry(): boolean {
    return this.moveMatching(1, (entry) => entry.linked === "xENFORCED");
  }

  prevXEnforcedEntry(): boolean {
    return this.moveMatching(-1, (entry) => entry.linked === "xENFORCED");
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
    if (!this.document) return this.getCurrentTranslation();
    this.adoptLiveDocument();
    const current = this.currentUndoState();
    const next = this.undo.undo(current);
    if (next !== current) this.replaceWithoutHistory(next);
    return next.translation;
  }

  redoEdit(): string {
    if (!this.document) return this.getCurrentTranslation();
    this.adoptLiveDocument();
    const current = this.currentUndoState();
    const next = this.undo.redo(current);
    if (next !== current) this.replaceWithoutHistory(next);
    return next.translation;
  }

  setFilter(filter: IEditorFilter): void {
    this.adoptLiveDocument();
    this.syncActiveEntry();
    this.entriesFilter = filter;
    this.rebuildVisibleEntries();
    if (this.displayedEntryIndex >= 0 && !this.visibleEntryIndices.includes(this.displayedEntryIndex)) {
      const next = this.visibleEntryIndices[0];
      if (next === undefined) {
        this.commitCurrentDocument(true);
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

  /**
   * Remember the first segment intersecting the scroll viewport. The offset is
   * relative to the viewport rather than scrollTop, so it survives prepending
   * variably-sized rendered segments.
   */
  captureScrollAnchor(
    viewportTop: number,
    candidates: readonly ScrollAnchorCandidate[],
  ): EditorScrollAnchor | null {
    const candidate =
      candidates.find(({ bottom }) => Number.isFinite(bottom) && bottom > viewportTop)
      ?? candidates.at(-1);
    if (!candidate || !Number.isFinite(candidate.top)) return null;
    return { key: candidate.key, offset: candidate.top - viewportTop };
  }

  /**
   * Return the scrollTop delta needed to put a stable segment back at its
   * pre-render viewport offset.
   */
  scrollAdjustmentForAnchor(
    anchor: EditorScrollAnchor | null,
    viewportTop: number,
    candidates: readonly ScrollAnchorCandidate[],
  ): number {
    if (!anchor) return 0;
    const candidate = candidates.find(({ key }) => key === anchor.key);
    if (!candidate || !Number.isFinite(candidate.top)) return 0;
    return candidate.top - viewportTop - anchor.offset;
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
    return this.moveMatching(delta, () => true);
  }

  private moveMatching(
    direction: -1 | 1,
    matches: (entry: LoadedEntry, index: number) => boolean,
  ): boolean {
    if (this.displayedEntryIndex < 0) return false;
    const visible = new Set(this.visibleEntryIndices);
    const target = findCyclicEntryIndex(
      this.entries,
      this.displayedEntryIndex,
      direction,
      (_entry, index) => visible.has(index),
      matches,
    );
    if (target === null) return false;
    const changed = target !== this.displayedEntryIndex;
    this.activateEntry(target);
    return changed;
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

  private replaceWithoutHistory(state: EditorUndoState): void {
    if (!this.document) {
      this.editor.replaceEditText(state.translation);
      return;
    }
    this.document = replaceDocumentText(this.document, state.translation);
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document);
    this.setCaretPosition(state.caret);
  }

  /**
   * Pull direct textarea/IME edits into the controller before a navigation or
   * commit can replace the active Document3.
   */
  private adoptLiveDocument(): void {
    if (!this.document) return;
    const live = this.textArea.getOmDocument();
    if (live.source !== this.document.source) return;
    if (this.textArea.isComposing()) this.textArea.commitComposition();
    this.document = this.textArea.getOmDocument();
  }

  private currentUndoState(): EditorUndoState {
    return {
      translation: this.getCurrentTranslation(),
      caret: this.getCurrentPositionInEntryTranslationInEditor(),
    };
  }

  private commitCurrentDocument(deactivate: boolean): void {
    if (!this.document || this.displayedEntryIndex < 0) return;
    this.adoptLiveDocument();
    this.syncActiveEntry();
    this.propagateCurrentDefaultTranslation();
    if (!deactivate || !this.document.editMode) return;
    this.document = commitDocument(this.document);
    this.refreshCurrentMarkers();
    this.textArea.setDocument(this.document, true);
    this.undo.undoStack = [];
    this.undo.redoStack = [];
  }

  private syncActiveEntry(): void {
    if (!this.document || this.displayedEntryIndex < 0) return;
    const entry = this.entries[this.displayedEntryIndex];
    if (entry) entry.translation = this.document.translation;
  }

  private propagateCurrentDefaultTranslation(): void {
    const active = this.entries[this.displayedEntryIndex];
    if (!active || active.isAlt || !this.document) return;
    for (let index = 0; index < this.entries.length; index += 1) {
      const entry = this.entries[index]!;
      if (
        index === this.displayedEntryIndex
        || entry.source !== active.source
        || entry.isAlt
      ) {
        continue;
      }
      entry.translation = this.document.translation;
      entry.translated = this.document.translation.length > 0;
      this.markers.invalidate(this.entryKey(index, entry));
    }
  }

  private markerInput(entry: LoadedEntry, active: boolean): MarkerInput {
    return {
      sourceText: entry.source,
      translationText: entry.translation,
      isActive: active,
      isAlt: entry.isAlt,
      fromAuto: entry.fromAuto,
      fromMt: entry.fromMt,
      linked: entry.linked,
      protectedParts: entry.protectedParts,
    };
  }

  private entryKey(index: number, entry: LoadedEntry): string {
    return entry.key
      ? JSON.stringify(entry.key)
      : JSON.stringify({
          index,
          file: entry.file,
          source_text: entry.source,
          id: entry.id ?? null,
        });
  }

  async refreshCurrentMarkersAsync(): Promise<boolean> {
    if (!this.document || this.displayedEntryIndex < 0) return false;
    const entry = this.entries[this.displayedEntryIndex];
    if (!entry) return false;
    const key = this.entryKey(this.displayedEntryIndex, entry);
    const input = this.markerInput(entry, true);
    const source = this.document.source;
    const translation = this.document.translation;
    await this.markers.processEntryAsync(key, input);
    const current = this.entries[this.displayedEntryIndex];
    if (
      !this.document
      || !current
      || this.entryKey(this.displayedEntryIndex, current) !== key
      || this.document.source !== source
      || this.document.translation !== translation
    ) {
      return false;
    }
    const marked = this.markers.applyToDocument(key, this.document, input);
    this.document = marked.document;
    this.markerSnapshot = marked.snapshot;
    this.textArea.setDocument(this.document, true);
    return true;
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
