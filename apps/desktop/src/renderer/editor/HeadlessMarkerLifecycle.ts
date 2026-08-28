// SPDX-License-Identifier: GPL-3.0-or-later

import type { Document3State } from "./Document3";
import { HeadlessLoadedWindow } from "./HeadlessLoadedWindow";
import { MarkerController, type MarkerSnapshot } from "./MarkerController";
import type { ProtectedRange } from "./EditorTextArea3";
import type { MarkerInput, MarkerProvider, ProtectedPart } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";
import type { EntryKeyDto } from "../lib/types";

export type HeadlessMarkerEntry = {
  key?: EntryKeyDto;
  file: string;
  source: string;
  translation: string;
  id?: string;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
  linked?: "xICE" | "x100PC" | "xAUTO" | "xENFORCED";
  protectedParts?: ProtectedPart[];
};

export type HeadlessMarkerPageEntry = {
  key: string;
  index: number;
  entryNumber: number;
  file: string;
  source: string;
  translation: string;
  active: boolean;
  marks: Mark[];
};

export type HeadlessMarkerState = {
  entries: readonly HeadlessMarkerEntry[];
  activeIndex: number;
  document: Document3State | null;
};

/**
 * Connects a headless loaded page to `MarkerController`.
 *
 * The loaded-window generation is the publication fence: edits, navigation,
 * filtering, reload, or contraction invalidate all asynchronous work whose
 * entry is no longer represented by the same page.
 */
export class HeadlessMarkerLifecycle {
  readonly markers: MarkerController;
  snapshot: MarkerSnapshot | null = null;

  constructor(
    readonly loadedWindow: HeadlessLoadedWindow,
    markers = new MarkerController(),
  ) {
    this.markers = markers;
  }

  registerPluginMarker(name: string, marker: MarkerProvider): void {
    this.markers.registerPluginMarker(name, marker);
  }

  unregisterPluginMarker(name: string): boolean {
    return this.markers.unregisterPluginMarker(name);
  }

  remarkOneMarker(name: string): void {
    this.markers.remarkOneMarker(name);
  }

  invalidateAll(invalidateWindow = true): void {
    this.markers.invalidate();
    if (invalidateWindow) this.loadedWindow.invalidate();
    this.snapshot = null;
  }

  invalidateEntry(index: number, entry: HeadlessMarkerEntry): void {
    this.markers.invalidate(this.entryKey(index, entry));
  }

  clearSnapshot(): void {
    this.snapshot = null;
  }

  synchronizeLoadedEntries(entries: readonly HeadlessMarkerEntry[]): void {
    const keys = this.loadedWindow.loadedIndices().map((index) =>
      this.entryKey(index, entries[index]!)
    );
    if (this.loadedWindow.synchronizeMarkerKeys(keys)) {
      this.markers.retainEntries(keys);
    }
  }

  page(
    entries: readonly HeadlessMarkerEntry[],
    activeIndex: number,
  ): HeadlessMarkerPageEntry[] {
    this.synchronizeLoadedEntries(entries);
    return this.loadedWindow.loadedIndices().map((index) => {
      const entry = entries[index]!;
      const active = index === activeIndex;
      const snapshot = active && this.snapshot
        ? this.snapshot
        : this.markers.processEntry(
          this.entryKey(index, entry),
          this.markerInput(entry, active),
        );
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

  decorateCurrent(
    entries: readonly HeadlessMarkerEntry[],
    activeIndex: number,
    document: Document3State,
  ): Document3State {
    const entry = entries[activeIndex];
    if (!entry) {
      this.snapshot = null;
      return document;
    }
    const marked = this.markers.applyToDocument(
      this.entryKey(activeIndex, entry),
      document,
      this.markerInput(entry, true),
    );
    this.snapshot = marked.snapshot;
    return marked.document;
  }

  documentPresentation(document: Document3State): {
    document: Document3State;
    protectedRanges: ProtectedRange[];
  } {
    return {
      document,
      protectedRanges: this.snapshot?.marks.flatMap((mark): ProtectedRange[] =>
        mark.entryPart === "TRANSLATION" && mark.painter === "protected"
          ? [{
              start: mark.startOffset,
              end: mark.endOffset,
              tooltip: mark.toolTipText,
            }]
          : []
      ) ?? [],
    };
  }

  async refreshCurrentAsync(
    getState: () => HeadlessMarkerState,
  ): Promise<Document3State | null> {
    const initial = getState();
    if (!initial.document || initial.activeIndex < 0) return null;
    const entry = initial.entries[initial.activeIndex];
    if (!entry) return null;
    const key = this.entryKey(initial.activeIndex, entry);
    const input = this.markerInput(entry, true);
    const source = initial.document.source;
    const translation = initial.document.translation;
    await this.markers.processEntryAsync(key, input);

    const current = getState();
    const currentEntry = current.entries[current.activeIndex];
    if (
      !current.document
      || !currentEntry
      || current.activeIndex !== initial.activeIndex
      || this.entryKey(current.activeIndex, currentEntry) !== key
      || current.document.source !== source
      || current.document.translation !== translation
    ) {
      return null;
    }
    const marked = this.markers.applyToDocument(key, current.document, input);
    this.snapshot = marked.snapshot;
    return marked.document;
  }

  async refreshPageAsync(
    getState: () => HeadlessMarkerState,
  ): Promise<{ accepted: boolean; document: Document3State | null }> {
    const initial = getState();
    const generation = this.loadedWindow.currentGeneration();
    const jobs = this.loadedWindow.loadedIndices().map((index) => {
      const entry = initial.entries[index]!;
      return {
        index,
        key: this.entryKey(index, entry),
        input: this.markerInput(entry, index === initial.activeIndex),
      };
    });
    if (jobs.length === 0) {
      return { accepted: false, document: initial.document };
    }
    await Promise.all(jobs.map(({ key, input }) =>
      this.markers.processEntryAsync(key, input)
    ));
    if (generation !== this.loadedWindow.currentGeneration()) {
      return { accepted: false, document: getState().document };
    }

    const current = getState();
    const loaded = this.loadedWindow.loadedIndices();
    if (
      loaded.length !== jobs.length
      || loaded.some((index, offset) => {
        const entry = current.entries[index];
        const job = jobs[offset];
        return !entry
          || !job
          || index !== job.index
          || this.entryKey(index, entry) !== job.key
          || JSON.stringify(this.markerInput(entry, index === current.activeIndex))
            !== JSON.stringify(job.input);
      })
    ) {
      return { accepted: false, document: current.document };
    }

    let document = current.document;
    const active = jobs.find(({ index }) => index === current.activeIndex);
    const activeEntry = current.entries[current.activeIndex];
    if (
      active
      && activeEntry
      && document
      && document.source === active.input.sourceText
      && document.translation === active.input.translationText
    ) {
      const marked = this.markers.applyToDocument(
        active.key,
        document,
        active.input,
      );
      document = marked.document;
      this.snapshot = marked.snapshot;
    }
    return { accepted: true, document };
  }

  entryKey(index: number, entry: HeadlessMarkerEntry): string {
    return entry.key
      ? JSON.stringify(entry.key)
      : JSON.stringify({
          index,
          file: entry.file,
          source_text: entry.source,
          id: entry.id ?? null,
        });
  }

  markerInput(entry: HeadlessMarkerEntry, active: boolean): MarkerInput {
    return {
      sourceText: entry.source,
      translationText: entry.translation,
      isActive: active,
      isAlt: entry.isAlt,
      fromAuto: entry.fromAuto,
      fromMt: entry.fromMt,
      linked: entry.linked,
      protectedParts: entry.protectedParts,
      entryKey: entry.key,
    };
  }
}
