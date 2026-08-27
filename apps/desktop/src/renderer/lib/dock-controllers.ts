import type {
  DictHitDto,
  EntryDto,
  EntryKeyDto,
  GlossaryHitDto,
  MatchDto,
  MtSuggestionDto,
} from "./types";

export type DockEditTarget = {
  getCurrentTranslation(): string;
  replaceEditText(text: string): void;
  insertText(text: string): void;
};

function boundedIndex(index: number, length: number): number {
  if (length === 0) return -1;
  return Math.max(0, Math.min(Math.trunc(index), length - 1));
}

export class LatestDockRequest<T> {
  private generation = 0;
  private pending: AbortController | null = null;

  cancel(): void {
    this.generation += 1;
    this.pending?.abort();
    this.pending = null;
  }

  isPending(): boolean {
    return this.pending !== null;
  }

  /**
   * Only the newest result may publish. AbortSignal also lets multi-step dock
   * loads stop before issuing their next sidecar request.
   */
  async run(
    load: (signal: AbortSignal) => Promise<T>,
    publish: (value: T) => void,
  ): Promise<boolean> {
    this.cancel();
    const generation = this.generation;
    const request = new AbortController();
    this.pending = request;
    try {
      const value = await load(request.signal);
      if (
        request.signal.aborted
        || generation !== this.generation
        || this.pending !== request
      ) {
        return false;
      }
      publish(value);
      return true;
    } catch (error) {
      if (request.signal.aborted || generation !== this.generation) return false;
      throw error;
    } finally {
      if (this.pending === request) this.pending = null;
    }
  }
}

export type DockNotificationTone = "hit" | "miss";

export class DockNotificationController {
  constructor(
    private notifyHits = true,
    private notifyMisses = false,
  ) {}

  setNotifyHits(enabled: boolean): void {
    this.notifyHits = enabled;
  }

  setNotifyMisses(enabled: boolean): void {
    this.notifyMisses = enabled;
  }

  getSettings(): { hits: boolean; misses: boolean } {
    return { hits: this.notifyHits, misses: this.notifyMisses };
  }

  signal(resultCount: number): DockNotificationTone | null {
    if (resultCount > 0) return this.notifyHits ? "hit" : null;
    return this.notifyMisses ? "miss" : null;
  }
}

export type DockMenuItem = {
  id: string;
  label: string;
  checked?: boolean;
  disabled?: boolean;
  separatorBefore?: boolean;
  action: () => void;
};

export type DockPopupSnapshot = {
  open: boolean;
  x: number;
  y: number;
  items: DockMenuItem[];
};

export class DockPopupController {
  private items: DockMenuItem[] = [];
  private x = 0;
  private y = 0;
  private opened = false;

  update(items: readonly DockMenuItem[]): void {
    this.items = items.map((item) => ({ ...item }));
  }

  open(x: number, y: number): DockPopupSnapshot {
    this.x = Math.max(0, x);
    this.y = Math.max(0, y);
    this.opened = true;
    return this.snapshot();
  }

  close(): DockPopupSnapshot {
    this.opened = false;
    return this.snapshot();
  }

  invoke(id: string): boolean {
    const item = this.items.find((candidate) => candidate.id === id);
    if (!item || item.disabled) return false;
    item.action();
    this.opened = false;
    return true;
  }

  snapshot(): DockPopupSnapshot {
    return {
      open: this.opened,
      x: this.x,
      y: this.y,
      items: this.items.map((item) => ({ ...item })),
    };
  }
}

/**
 * Stateful fuzzy-match selection, mirroring MatchesTextArea instead of
 * treating every pointer click as an immediate overwrite.
 */
export class MatchesController {
  readonly matches: MatchDto[];
  activeMatch: number;

  constructor(matches: readonly MatchDto[], activeMatch = 0) {
    this.matches = matches
      .map((match) => ({ ...match }))
      .sort((left, right) => right.score - left.score);
    this.activeMatch = boundedIndex(activeMatch, this.matches.length);
  }

  select(index: number): number {
    if (index >= 0 && index < this.matches.length) this.activeMatch = index;
    return this.activeMatch;
  }

  next(): number {
    return this.select(this.activeMatch + 1);
  }

  previous(): number {
    return this.select(this.activeMatch - 1);
  }

  getActiveMatch(): MatchDto | null {
    return this.matches[this.activeMatch] ?? null;
  }

  apply(target: DockEditTarget, mode: "insert" | "overwrite", index = this.activeMatch): boolean {
    const match = this.matches[index];
    if (!match) return false;
    this.activeMatch = index;
    if (mode === "insert") target.insertText(match.translation);
    else target.replaceEditText(match.translation);
    return true;
  }
}

export type GlossaryDisplayEntry = Pick<GlossaryHitDto, "source" | "target"> & {
  comment?: string;
};

export function decodeGlossaryComment(comment = ""): string {
  try {
    return decodeURI(comment);
  } catch {
    return comment;
  }
}

/** Plain-text rendering used by the glossary dock and accessibility labels. */
export function renderGlossaryText(entries: readonly GlossaryDisplayEntry[]): string {
  return entries
    .map((entry) => {
      const comment = decodeGlossaryComment(entry.comment);
      return `${entry.source} = ${entry.target}${comment ? `\n1. ${comment}` : ""}`;
    })
    .join("");
}

export class GlossaryController {
  readonly entries: GlossaryDisplayEntry[];

  constructor(entries: readonly GlossaryDisplayEntry[]) {
    this.entries = entries.map((entry) => ({ ...entry }));
  }

  getText(): string {
    return renderGlossaryText(this.entries);
  }

  insertTarget(target: DockEditTarget, index: number): boolean {
    const entry = this.entries[index];
    if (!entry) return false;
    target.insertText(entry.target);
    return true;
  }
}

/** Editable note document with the per-entry undo lifecycle of NotesTextArea. */
export class NotesController {
  private value: string | null;
  private undoStack: Array<string | null> = [];
  private redoStack: Array<string | null> = [];

  constructor(text: string | null = null) {
    this.value = noteText(text ?? "");
  }

  activate(text: string | null): void {
    this.value = noteText(text ?? "");
    this.clearHistory();
  }

  set(text: string): void {
    const next = noteText(text);
    if (next === this.value) return;
    this.undoStack.push(this.value);
    this.value = next;
    this.redoStack = [];
  }

  clear(): void {
    this.value = null;
    this.clearHistory();
  }

  clearHistory(): void {
    this.undoStack = [];
    this.redoStack = [];
  }

  undo(): string | null {
    const previous = this.undoStack.pop();
    if (previous === undefined) return this.value;
    this.redoStack.push(this.value);
    this.value = previous;
    return this.value;
  }

  redo(): string | null {
    const next = this.redoStack.pop();
    if (next === undefined) return this.value;
    this.undoStack.push(this.value);
    this.value = next;
    return this.value;
  }

  get(): string | null {
    return this.value;
  }
}

/** Compatibility name used by the Java-golden test surface. */
export class NotesDocument extends NotesController {}

/** Java NotesTextArea.getNoteText: empty text represents no note. */
export function noteText(value: string): string | null {
  return value === "" ? null : value;
}

export type CommentProvider<T> = (entry: T) => string | null;

/** Priority-ordered comment providers corresponding to CommentsTextArea. */
export class CommentsController<T> {
  private providers: Array<{ provider: CommentProvider<T>; priority: number }> = [];

  addProvider(provider: CommentProvider<T>, priority: number): void {
    this.providers.push({ provider, priority });
    this.providers.sort((left, right) => left.priority - right.priority);
  }

  removeProvider(provider: CommentProvider<T>): boolean {
    const index = this.providers.findIndex((item) => item.provider === provider);
    if (index < 0) return false;
    this.providers.splice(index, 1);
    return true;
  }

  render(entry: T): string {
    return this.providers
      .map(({ provider }) => provider(entry))
      .filter((comment): comment is string => comment !== null)
      .join("");
  }
}

export function entryComment(entry: EntryDto): string {
  const lines: string[] = [];
  if (entry.key.id !== null) lines.push(`ID ${entry.key.id}`);
  if (entry.key.path !== null) lines.push(`Path ${entry.key.path.replace(/\\n/g, "\n")}`);
  const sourceTranslation = entry.properties.find(([key]) => key === "translation")?.[1];
  if (sourceTranslation !== undefined) lines.push(`Translation ${sourceTranslation}`);
  if (entry.comment) lines.push(`Comment\n${entry.comment}`);
  return lines.length > 0 ? `${lines.join("\n")}\n` : "";
}

export type MultipleTranslationRow = {
  index: number;
  key: EntryKeyDto;
  translation: string;
  file: string;
  id: string;
  previous: string | null;
  next: string | null;
  isDefault: boolean;
};

export type MultipleTranslationTarget = DockEditTarget & {
  commitTranslationVariant(defaultTranslation: boolean): void | Promise<void>;
  gotoEntry(source: string, key: EntryKeyDto): boolean | void | Promise<boolean | void>;
};

export class MultipleTranslationsController {
  readonly source: string | null;
  readonly rows: MultipleTranslationRow[];

  constructor(entries: readonly EntryDto[], activeIndex: number) {
    const active = entries[activeIndex];
    this.source = active?.source ?? null;
    const rows = active
      ? entries
        .filter((entry) => entry.source === active.source)
        .map((entry) => ({
          index: entry.index,
          key: { ...entry.key },
          translation: entry.translation,
          file: entry.file,
          id: entry.id,
          previous: entry.key.prev,
          next: entry.key.next,
          isDefault: entry.default_translation,
        }))
      : [];
    this.rows = rows.length === 1 && rows[0]?.isDefault ? [] : rows;
  }

  replace(target: DockEditTarget, rowIndex: number): boolean {
    const row = this.rows[rowIndex];
    if (!row) return false;
    target.replaceEditText(row.translation);
    return true;
  }

  makeDefault(target: MultipleTranslationTarget, rowIndex: number): boolean {
    if (!this.replace(target, rowIndex)) return false;
    void target.commitTranslationVariant(true);
    return true;
  }

  goto(target: MultipleTranslationTarget, rowIndex: number): boolean {
    const row = this.rows[rowIndex];
    if (!row || this.source === null) return false;
    void target.gotoEntry(this.source, row.key);
    return true;
  }
}

/** Sorted MT results with Java's cyclic "displayed translation" selection. */
export class MachineTranslateController {
  readonly results: MtSuggestionDto[];
  selectedIndex: number;

  constructor(results: readonly MtSuggestionDto[], selectedIndex = -1) {
    this.results = results
      .map((result) => ({ ...result }))
      .sort((left, right) => left.engine.localeCompare(right.engine));
    this.selectedIndex = selectedIndex < 0
      ? -1
      : boundedIndex(selectedIndex, this.results.length);
  }

  select(index: number): number {
    if (index >= 0 && index < this.results.length) this.selectedIndex = index;
    return this.selectedIndex;
  }

  cycle(): MtSuggestionDto | null {
    if (this.results.length === 0) return null;
    this.selectedIndex = (this.selectedIndex + 1) % this.results.length;
    return this.results[this.selectedIndex]!;
  }

  getSelected(): MtSuggestionDto | null {
    return this.results[this.selectedIndex] ?? null;
  }

  apply(target: DockEditTarget, mode: "insert" | "overwrite", index = this.selectedIndex): boolean {
    const result = this.results[index];
    if (!result) return false;
    this.selectedIndex = index;
    if (mode === "insert") target.insertText(result.text);
    else target.replaceEditText(result.text);
    return true;
  }
}

/** Dictionary display ordering and exact/fuzzy article focus. */
export class DictionaryController {
  readonly entries: DictHitDto[];
  selectedIndex = -1;

  constructor(entries: readonly DictHitDto[]) {
    this.entries = entries
      .map((entry) => ({ ...entry }))
      .sort((left, right) =>
        left.word.localeCompare(right.word)
        || left.definition.localeCompare(right.definition)
        || left.source.localeCompare(right.source)
      );
  }

  focusWord(word: string, stemmedWords: readonly string[] = []): number {
    const candidates = [word, ...stemmedWords].map((candidate) => candidate.toLocaleLowerCase());
    this.selectedIndex = this.entries.findIndex((entry) =>
      candidates.includes(entry.word.toLocaleLowerCase())
    );
    return this.selectedIndex;
  }
}

export type SegmentPropertyRow = {
  key: string;
  value: string;
  notify: boolean;
};

/**
 * Structured SegmentPropertiesArea data. The renderer keeps raw keys so list
 * and table views can share the same notification selection.
 */
export class SegmentPropertiesController {
  private notifyKeys: Set<string>;

  constructor(notifyKeys: readonly string[] = []) {
    this.notifyKeys = new Set(notifyKeys);
  }

  toggleNotification(key: string, enabled: boolean): void {
    if (enabled) this.notifyKeys.add(key);
    else this.notifyKeys.delete(key);
  }

  getNotificationKeys(): string[] {
    return [...this.notifyKeys];
  }

  rows(entry: EntryDto | undefined): SegmentPropertyRow[] {
    if (!entry) return [];
    const pairs: Array<[string, string | null | undefined]> = [
      ["hasComment", entry.comment ? "yes" : null],
      ["file", entry.key.file],
      ["id", entry.key.id],
      ["path", entry.key.path],
      ["hasNote", entry.note ? "yes" : null],
      ["isAlt", entry.default_translation ? null : "yes"],
      ["revision", String(entry.revision)],
      ...entry.properties,
    ];
    const seen = new Set<string>();
    return pairs.flatMap(([key, value]) => {
      if (value === null || value === undefined || seen.has(key)) return [];
      seen.add(key);
      return [{
        key,
        value,
        notify: this.notifyKeys.has(key),
      }];
    });
  }

  notifiedRowIndices(entry: EntryDto | undefined): number[] {
    return this.rows(entry).flatMap((row, index) => row.notify ? [index] : []);
  }
}
