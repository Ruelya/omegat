/** Java `org.omegat.gui.editor.EditorController` — IEditor implementation host. */
import {
  replaceEditText as replaceDocumentText,
  type Document3State,
} from "./Document3";
import {
  EditorDocumentLifecycle,
  type EditorCaretPosition,
} from "./EditorDocumentLifecycle";
import {
  HeadlessMarkerLifecycle,
  type HeadlessMarkerPageEntry,
} from "./HeadlessMarkerLifecycle";
import { IEditor } from "./IEditor";
import { makeFilter, type IEditorFilter } from "./IEditorFilter";
import type { MarkerSnapshot } from "./MarkerController";
import { SegmentHistory } from "./SegmentHistory";
import { TranslationUndoManager } from "./TranslationUndoManager";
import { changeCase as changeEditorCase, getWordBoundary, type ChangeCaseMode } from "./EditorUtils";
import {
  handleEditorFileDrop,
  type EditorFileDrop,
  type EditorFileDropHandlers,
  type EditorFileDropResult,
} from "./EditorFileDrop";
import {
  findCyclicEntryIndex,
  findEntryBySourceAndKey,
  findEntryInFile,
  rebindEntryAfterReload,
} from "./EditorNavigation";
import { HeadlessLoadedWindow } from "./HeadlessLoadedWindow";
import type { MarkerProvider, ProtectedPart } from "./mark/IMarker";
import type { EntryKeyDto, IssueDto } from "../lib/types";

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

export type LoadedPageEntry = HeadlessMarkerPageEntry;

export type ScrollAnchorCandidate = {
  key: string;
  top: number;
  bottom: number;
};

export type EditorScrollAnchor = {
  key: string;
  offset: number;
};

export type { EditorCaretPosition } from "./EditorDocumentLifecycle";

export type {
  EditorFileDrop,
  EditorFileDropHandlers,
  EditorFileDropResult,
} from "./EditorFileDrop";

export function issuesForEntryOnLeave(
  entry: Pick<LoadedEntry, "file">,
  issues: readonly IssueDto[],
  enabled = true,
): IssueDto[] {
  return enabled
    ? issues
      .filter((issue) => issue.file === entry.file)
      .map((issue) => ({ ...issue }))
    : [];
}

type EditorUndoState = {
  translation: string;
  caret: EditorCaretPosition;
};

export class EditorController {
  readonly editor = IEditor;
  readonly documents = new EditorDocumentLifecycle();
  readonly textArea = this.documents.textArea;
  readonly loadedWindow = new HeadlessLoadedWindow();
  readonly markerLifecycle = new HeadlessMarkerLifecycle(this.loadedWindow);
  readonly markers = this.markerLifecycle.markers;
  readonly undo = new TranslationUndoManager<EditorUndoState>();
  readonly history = new SegmentHistory();
  displayedFileIndex = 0;
  previousDisplayedFileIndex = 0;
  displayedEntryIndex = -1;
  currentFile: string | null = null;
  currentEntryNumber = 0;
  entries: LoadedEntry[] = [];
  sourceLangIsRTL = false;
  targetLangIsRTL = false;
  leaveIssues: IssueDto[] = [];
  targetLocale = "en";
  private entriesFilter: IEditorFilter = makeFilter("none");
  private filterOriginIndex = 0;

  get document(): Document3State | null {
    return this.documents.document;
  }

  set document(document: Document3State | null) {
    this.documents.setCurrent(document);
  }

  get markerSnapshot(): MarkerSnapshot | null {
    return this.markerLifecycle.snapshot;
  }

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
    this.bindDocumentToTextArea();
  }

  /** Java `replacePartOfText`: offsets are UTF-16 positions in the translation. */
  replacePartOfText(text: string, start: number, end: number): boolean {
    if (!this.document) return false;
    this.adoptLiveDocument();
    const length = this.document.translation.length;
    if (
      !Number.isInteger(start)
      || !Number.isInteger(end)
      || start < 0
      || end < start
      || end > length
    ) {
      throw new RangeError(`translation range ${start}..${end} outside 0..${length}`);
    }
    this.bindDocumentToTextArea(true);
    this.textArea.setSelection(
      this.document.translationStart + start,
      this.document.translationStart + end,
    );
    const before = this.currentUndoState();
    if (!this.textArea.replaceSelection(text)) return false;
    this.undo.remember(before);
    this.document = this.textArea.getOmDocument();
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.bindDocumentToTextArea(true);
    return true;
  }

  insertText(text: string): void {
    if (!this.document) {
      this.editor.insertText(text);
      return;
    }
    this.adoptLiveDocument();
    this.bindDocumentToTextArea(true);
    this.textArea.clampSelectionToTranslation();
    const before = this.currentUndoState();
    if (!this.textArea.replaceSelection(text)) return;
    this.undo.remember(before);
    this.document = this.textArea.getOmDocument();
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.bindDocumentToTextArea(true);
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

  getCurrentTargetFile(targetRoot: string): string | null {
    const file = this.getCurrentFile();
    if (!file) return null;
    const separator = targetRoot.includes("\\") && !targetRoot.includes("/") ? "\\" : "/";
    const root = targetRoot.replace(/[\\/]+$/, "");
    const relative = file.replace(/^[\\/]+/, "").replace(/[\\/]/g, separator);
    return `${root}${separator}${relative}`;
  }

  getCurrentEntryNumber(): number {
    return this.currentEntryNumber;
  }

  getOmDocument(): Document3State | null {
    return this.document;
  }

  getPositionInEntryTranslation(position: number): number {
    return this.documents.getPositionInEntryTranslation(position);
  }

  getCurrentPositionInEntryTranslation(): number {
    return this.documents.getCurrentPositionInEntryTranslation();
  }

  getCurrentPositionInEntryTranslationInEditor(): EditorCaretPosition {
    return this.documents.getCaretPosition();
  }

  setCaretPosition(position: EditorCaretPosition): void {
    if (!this.document?.editMode) return;
    this.bindDocumentToTextArea(true);
    this.documents.setCaretPosition(position);
  }

  getSelectedText(): string {
    if (!this.document?.editMode) return "";
    if (this.textArea.getOmDocument() !== this.document) {
      this.bindDocumentToTextArea(true);
    }
    return this.documents.getSelectedText();
  }

  setTargetLocale(locale: string): void {
    this.targetLocale = locale || "und";
    this.textArea.setTargetLocale(this.targetLocale);
  }

  /**
   * Change the selected text, or the word touching a collapsed caret, through
   * the active EditorTextArea3/Document3 UTF-16 range.
   */
  changeCase(mode: ChangeCaseMode): boolean {
    if (!this.document?.editMode) return false;
    this.adoptLiveDocument();
    this.bindDocumentToTextArea(true);
    const doc = this.document;
    let start = Math.max(doc.translationStart, this.textArea.getSelectionStart());
    let end = Math.min(doc.translationEnd, this.textArea.getSelectionEnd());
    if (start > end) return false;
    if (start === end) {
      const caret = Math.max(0, Math.min(
        this.textArea.getCaretPosition() - doc.translationStart,
        doc.translation.length,
      ));
      const probe = caret > 0 ? caret - 1 : caret;
      start = doc.translationStart
        + getWordBoundary(this.targetLocale, doc.translation, probe, false);
      end = doc.translationStart
        + getWordBoundary(this.targetLocale, doc.translation, probe, true);
    }
    start = Math.max(doc.translationStart, Math.min(start, doc.translationEnd));
    end = Math.max(start, Math.min(end, doc.translationEnd));
    this.textArea.setSelection(start, end);
    const selected = doc.fullText.slice(start, end);
    const replacement = changeEditorCase(selected, mode, this.targetLocale);
    if (selected === replacement) return false;

    const before = this.currentUndoState();
    if (!this.textArea.replaceSelection(replacement)) return false;
    this.undo.remember(before);
    this.document = this.textArea.getOmDocument();
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.bindDocumentToTextArea(true);
    this.textArea.setSelection(start, start + replacement.length);
    return true;
  }

  /**
   * Rebuild the active segment after an external fixer. When the active entry
   * was fixed, its authoritative entry value replaces the dirty editor text
   * without first committing that stale draft.
   */
  refreshViewAfterFix(fixedEntries: readonly number[] | null = null): boolean {
    const current = this.getCurrentEntryNumber();
    const activeWasFixed = fixedEntries === null || fixedEntries.includes(current);
    return this.refreshView(!activeWasFixed);
  }

  refreshView(doCommit = true): boolean {
    if (!this.document || this.displayedEntryIndex < 0) return false;
    const caret = this.getCurrentPositionInEntryTranslationInEditor();
    const index = this.displayedEntryIndex;
    if (doCommit) this.commitCurrentDocument(true);
    this.openEntry(index, false, caret);
    return true;
  }

  refreshEntries(entryNumbers: ReadonlySet<number>): void {
    let refreshActive = false;
    entryNumbers.forEach((entryNumber) => {
      const index = entryNumber - 1;
      const entry = this.entries[index];
      if (!entry) return;
      this.markerLifecycle.invalidateEntry(index, entry);
      if (index === this.displayedEntryIndex) refreshActive = true;
    });
    if (refreshActive) {
      this.refreshView(false);
    } else if (entryNumbers.size > 0) {
      this.loadedWindow.invalidate();
    }
  }

  getCurrentEntry(): LoadedEntry | null {
    return this.entries[this.displayedEntryIndex] ?? null;
  }

  checkIssuesOnLeave(
    entry: LoadedEntry,
    _entryIndex: number,
    issues: readonly IssueDto[],
    enabled = true,
  ): IssueDto[] {
    this.leaveIssues = issuesForEntryOnLeave(entry, issues, enabled);
    return this.leaveIssues.map((issue) => ({ ...issue }));
  }

  async handleFileDrop(
    drop: EditorFileDrop,
    projectLoaded: boolean,
    handlers: EditorFileDropHandlers,
  ): Promise<EditorFileDropResult> {
    return handleEditorFileDrop(drop, projectLoaded, handlers);
  }

  setCurrentTranslationVariant(defaultTranslation: boolean): void {
    const entry = this.getCurrentEntry();
    if (!entry) return;
    entry.isAlt = !defaultTranslation;
    this.refreshCurrentMarkers();
  }

  registerPluginMarker(name: string, marker: MarkerProvider): void {
    this.markerLifecycle.registerPluginMarker(name, marker);
    this.refreshCurrentMarkers();
  }

  unregisterPluginMarker(name: string): boolean {
    const removed = this.markerLifecycle.unregisterPluginMarker(name);
    if (removed) this.refreshCurrentMarkers();
    return removed;
  }

  remarkOneMarker(name: string): void {
    this.markerLifecycle.remarkOneMarker(name);
    this.refreshCurrentMarkers();
  }

  loadProject(entries: LoadedEntry[], preferredEntryNumber = 1): void {
    this.commitCurrentDocument(true);
    this.document = null;
    this.displayedEntryIndex = -1;
    this.entries = entries.map((entry) => ({ ...entry }));
    this.markerLifecycle.invalidateAll();
    this.rebuildVisibleEntries();
    this.history.back = [];
    this.history.forward = [];
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    if (this.loadedWindow.visibleIndices().length === 0) {
      this.clearActiveView(Math.max(0, preferredEntryNumber - 1));
      return;
    }
    const requested = Math.max(0, Math.min(preferredEntryNumber - 1, this.entries.length - 1));
    const initial = this.loadedWindow.contains(requested)
      ? requested
      : this.loadedWindow.firstVisible()!;
    this.activateEntry(initial);
  }

  /**
   * Rebuild the editor after a project reload while retaining the active
   * complete EntryKey and its translation-relative caret/selection. Incoming
   * entries remain authoritative: the caller commits the old entry before
   * asking the sidecar to reload.
   *
   * @returns whether the former complete EntryKey still exists after reload.
   */
  reloadProject(entries: readonly LoadedEntry[]): boolean {
    this.adoptLiveDocument();
    const previousIndex = this.displayedEntryIndex >= 0
      ? this.displayedEntryIndex
      : this.filterOriginIndex;
    const previousEntry = this.entries[this.displayedEntryIndex];
    const previousKey = previousEntry
      ? this.entryKey(this.displayedEntryIndex, previousEntry)
      : null;
    const caret = this.document
      ? this.getCurrentPositionInEntryTranslationInEditor()
      : { position: 0 };
    this.commitCurrentDocument(true);

    this.entries = entries.map((entry) => ({ ...entry }));
    this.markerLifecycle.invalidateAll();
    this.rebuildVisibleEntries();
    this.history.back = [];
    this.history.forward = [];
    this.undo.undoStack = [];
    this.undo.redoStack = [];

    const rebound = rebindEntryAfterReload(
      this.entries,
      previousIndex,
      (entry, index) =>
        previousKey !== null && this.entryKey(index, entry) === previousKey,
    );
    const reboundIndex = rebound.exact ? rebound.index : -1;
    if (this.loadedWindow.visibleIndices().length === 0) {
      this.clearActiveView(reboundIndex >= 0 ? reboundIndex : previousIndex);
      return reboundIndex >= 0;
    }
    const reboundVisible =
      reboundIndex >= 0 && this.loadedWindow.contains(reboundIndex);
    const anchor = reboundIndex >= 0 ? reboundIndex : previousIndex;
    const target = reboundVisible
      ? reboundIndex
      : this.loadedWindow.findVisible((index) => index >= anchor)
        ?? this.loadedWindow.firstVisible()!;
    this.openEntry(target, false, reboundVisible ? caret : { position: 0 });
    return reboundIndex >= 0;
  }

  loadEmptyProject(): void {
    this.commitCurrentDocument(true);
    this.entries = [];
    this.loadedWindow.clear();
    this.displayedFileIndex = 0;
    this.previousDisplayedFileIndex = 0;
    this.history.back = [];
    this.history.forward = [];
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    this.markerLifecycle.invalidateAll(false);
    this.clearActiveView(0);
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
    this.filterOriginIndex = index;
    this.currentFile = e.file;
    this.currentEntryNumber = index + 1;
    this.documents.activate(
      this.currentEntryNumber,
      e.source,
      e.translation,
      position,
      (document) => {
        this.document = document;
        this.refreshCurrentMarkers();
        return this.currentDocumentPresentation();
      },
    );
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    this.loadWindowAround(index);
    if (recordHistory && this.history.back.at(-1) !== this.currentEntryNumber) {
      this.history.visit(this.currentEntryNumber);
    }
  }

  gotoEntry(entryNumber: number): boolean {
    const index = entryNumber - 1;
    if (!this.loadedWindow.contains(index)) return false;
    const changed = index !== this.displayedEntryIndex;
    this.activateEntry(index);
    return changed;
  }

  /**
   * Java's source/key navigation used by fuzzy and multiple-translation panes.
   * A supplied key must match every EntryKey field; source-only navigation
   * resolves only a translated default, never an arbitrary alternative.
   */
  gotoEntryBySourceAndKey(
    source: string | null,
    key: EntryKeyDto | null = null,
  ): boolean {
    const index = findEntryBySourceAndKey(this.entries, source, key);
    if (index < 0 || !this.loadedWindow.contains(index)) return false;
    if (index === this.displayedEntryIndex) return true;
    this.activateEntry(index);
    return true;
  }

  gotoFile(file: string | number): boolean {
    const files = [...new Set(this.entries.map((entry) => entry.file))];
    const fileName = typeof file === "number" ? files[file] : file;
    if (fileName === undefined) {
      if (typeof file === "number") throw new RangeError("file index out of bounds");
      return false;
    }
    const index = findEntryInFile(
      this.entries,
      fileName,
      this.loadedWindow.visibleSet(),
    );
    if (index === null) return false;
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
    const hadActiveEntry = this.displayedEntryIndex >= 0;
    const previousIndex = hadActiveEntry
      ? this.displayedEntryIndex
      : this.filterOriginIndex;
    this.adoptLiveDocument();
    this.syncActiveEntry();
    const caret = this.document
      ? this.getCurrentPositionInEntryTranslationInEditor()
      : { position: 0 };
    if (hadActiveEntry) this.commitCurrentDocument(true);
    this.entriesFilter = filter;
    this.rebuildVisibleEntries();
    this.markerLifecycle.invalidateAll();

    if (this.loadedWindow.visibleIndices().length === 0) {
      this.clearActiveView(previousIndex);
      return;
    }
    const preserveCurrent =
      hadActiveEntry && this.loadedWindow.contains(previousIndex);
    const target = preserveCurrent
      ? previousIndex
      : this.loadedWindow.findVisible((index) =>
          hadActiveEntry ? index > previousIndex : index >= previousIndex
        ) ?? this.loadedWindow.firstVisible()!;
    this.openEntry(target, false, preserveCurrent ? caret : { position: 0 });
  }

  removeFilter(): void {
    this.setFilter(makeFilter("none"));
  }

  getFilter(): IEditorFilter {
    return this.entriesFilter;
  }

  getLoadedRange(): { first: number; last: number } {
    return this.loadedWindow.getRange();
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
    return this.markerLifecycle.page(this.entries, this.displayedEntryIndex);
  }

  setPageRadius(radius: number): void {
    this.loadedWindow.setRadius(radius, this.displayedEntryIndex);
    this.updateLoadedMarkerLifecycle();
  }

  loadUp(count: number): number {
    const loaded = this.loadedWindow.loadUp(count);
    this.updateLoadedMarkerLifecycle();
    return loaded;
  }

  loadDown(count: number): number {
    const loaded = this.loadedWindow.loadDown(count);
    this.updateLoadedMarkerLifecycle();
    return loaded;
  }

  hasMoreBefore(): boolean {
    return this.loadedWindow.hasMoreBefore();
  }

  hasMoreAfter(): boolean {
    return this.loadedWindow.hasMoreAfter();
  }

  private moveVisible(delta: -1 | 1): boolean {
    return this.moveMatching(delta, () => true);
  }

  private moveMatching(
    direction: -1 | 1,
    matches: (entry: LoadedEntry, index: number) => boolean,
  ): boolean {
    if (this.displayedEntryIndex < 0) return false;
    const visible = this.loadedWindow.visibleSet();
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
    this.loadedWindow.rebuild(
      this.entries,
      (entry) => this.entriesFilter.allowed(entry),
    );
  }

  private loadWindowAround(index: number): void {
    this.loadedWindow.around(index);
    this.updateLoadedMarkerLifecycle();
  }

  private updateLoadedMarkerLifecycle(): void {
    this.markerLifecycle.synchronizeLoadedEntries(this.entries);
  }

  private clearActiveView(originIndex: number): void {
    this.filterOriginIndex = Math.max(0, originIndex);
    this.document = null;
    this.currentFile = null;
    this.currentEntryNumber = 0;
    this.displayedEntryIndex = -1;
    this.loadedWindow.clearRange();
    this.markerLifecycle.clearSnapshot();
    this.undo.undoStack = [];
    this.undo.redoStack = [];
    this.documents.clear();
    this.updateLoadedMarkerLifecycle();
  }

  private replaceWithoutHistory(state: EditorUndoState): void {
    if (!this.document) {
      this.editor.replaceEditText(state.translation);
      return;
    }
    this.document = replaceDocumentText(this.document, state.translation);
    this.syncActiveEntry();
    this.refreshCurrentMarkers();
    this.bindDocumentToTextArea();
    this.setCaretPosition(state.caret);
  }

  /**
   * Pull direct textarea/IME edits into the controller before a navigation or
   * commit can replace the active Document3.
   */
  private adoptLiveDocument(): void {
    this.documents.adoptLiveDocument();
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
    this.documents.commit(true);
    this.refreshCurrentMarkers();
    this.bindDocumentToTextArea(true);
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
      this.markerLifecycle.invalidateEntry(index, entry);
    }
  }

  private entryKey(index: number, entry: LoadedEntry): string {
    return this.markerLifecycle.entryKey(index, entry);
  }

  async refreshCurrentMarkersAsync(): Promise<boolean> {
    const document = await this.markerLifecycle.refreshCurrentAsync(
      () => this.currentMarkerState(),
    );
    if (!document) return false;
    this.document = document;
    this.bindDocumentToTextArea(true);
    return true;
  }

  /**
   * Run asynchronous providers for every entry in the rendered page. A page
   * rebuild, filter, reload, edit, or navigation invalidates the captured
   * generation, including requests that belong to inactive entries.
   */
  async refreshLoadedPageMarkersAsync(): Promise<boolean> {
    const result = await this.markerLifecycle.refreshPageAsync(
      () => this.currentMarkerState(),
    );
    if (!result.accepted) return false;
    if (result.document) {
      this.document = result.document;
      this.bindDocumentToTextArea(true);
    }
    return result.accepted;
  }

  private refreshCurrentMarkers(): void {
    if (!this.document || this.displayedEntryIndex < 0) {
      this.markerLifecycle.clearSnapshot();
      return;
    }
    this.document = this.markerLifecycle.decorateCurrent(
      this.entries,
      this.displayedEntryIndex,
      this.document,
    );
  }

  private bindDocumentToTextArea(preserveSelection = false): void {
    if (!this.document) return;
    this.documents.applyPresentation(
      this.currentDocumentPresentation(),
      preserveSelection,
    );
  }

  private currentDocumentPresentation() {
    if (!this.document) throw new Error("active document required");
    return this.markerLifecycle.documentPresentation(this.document);
  }

  private currentMarkerState(): {
    entries: readonly LoadedEntry[];
    activeIndex: number;
    document: Document3State | null;
  } {
    return {
      entries: this.entries,
      activeIndex: this.displayedEntryIndex,
      document: this.document,
    };
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
