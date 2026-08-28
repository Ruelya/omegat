// SPDX-License-Identifier: GPL-3.0-or-later

import type { Document3State } from "./Document3";
import type { IEditorFilter } from "./IEditorFilter";
import { MarkerController } from "./MarkerController";
import type { MarkerInput, MarkerProvider, ProtectedPart } from "./mark/IMarker";
import type { EntryPart, Mark } from "./mark/Mark";
import type { EntryKeyDto } from "../lib/types";

export type RendererProjectionEntry = {
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
  linked?: "xICE" | "x100PC" | "xAUTO" | "xENFORCED";
  protectedParts?: ProtectedPart[];
};

export type RendererPageEntry = {
  key: string;
  index: number;
  entryNumber: number;
  file: string;
  source: string;
  translation: string;
  active: boolean;
  marks: Mark[];
};

export type RendererScrollAnchorCandidate = {
  key: string;
  top: number;
  bottom: number;
};

export type RendererScrollAnchor = {
  key: string;
  offset: number;
};

type RendererMarkerJob = {
  index: number;
  key: string;
  input: MarkerInput;
};

/**
 * Immutable page/Marker projection for the mounted React editor.
 *
 * Zustand owns the active EntryKey, Document3 and selection. This projection
 * owns only the currently rendered page and its request-scoped Marker cache;
 * it cannot commit, navigate, or adopt a second editable document.
 */
export class RendererPageProjection {
  readonly markers = new MarkerController();
  private visibleEntryIndices: number[] = [];
  private pageRadius = 8;
  private firstLoaded = -1;
  private lastLoaded = -1;
  private markerJobs: RendererMarkerJob[] = [];
  private pageGeneration = 0;

  setPageRadius(radius: number): void {
    this.pageRadius = Math.max(0, Math.floor(radius));
  }

  project(
    entries: readonly RendererProjectionEntry[],
    activeIndex: number,
    document: Document3State,
    filter: IEditorFilter,
  ): RendererPageEntry[] {
    this.visibleEntryIndices = entries.flatMap((entry, index) =>
      filter.allowed(entry) ? [index] : []
    );
    const safeIndex = Math.max(0, Math.min(activeIndex, entries.length - 1));
    const active = entries[safeIndex];
    if (!active || !this.visibleEntryIndices.includes(safeIndex)) {
      this.firstLoaded = -1;
      this.lastLoaded = -1;
      this.updateMarkerJobs([]);
      return [];
    }

    const visiblePosition = this.visibleEntryIndices.indexOf(safeIndex);
    const first = Math.max(0, visiblePosition - this.pageRadius);
    const last = Math.min(
      this.visibleEntryIndices.length - 1,
      visiblePosition + this.pageRadius,
    );
    const pageIndices = this.visibleEntryIndices.slice(first, last + 1);
    this.firstLoaded = pageIndices[0] ?? -1;
    this.lastLoaded = pageIndices.at(-1) ?? -1;
    const jobs = pageIndices.map((index): RendererMarkerJob => {
      const stored = entries[index]!;
      const entry = index === safeIndex
        ? {
            ...stored,
            source: document.source,
            translation: document.translation,
          }
        : stored;
      return {
        index,
        key: this.entryKey(index, entry),
        input: this.markerInput(entry, index === safeIndex),
      };
    });
    this.updateMarkerJobs(jobs);
    return jobs.map(({ index, key, input }) => {
      const entry = entries[index]!;
      return {
        key,
        index,
        entryNumber: index + 1,
        file: entry.file,
        source: input.sourceText ?? "",
        translation: input.translationText ?? "",
        active: input.isActive,
        marks: this.markers.processEntry(key, input).marks,
      };
    });
  }

  async refreshMarkersAsync(): Promise<boolean> {
    const generation = this.pageGeneration;
    const jobs = this.markerJobs.map(({ index, key, input }) => ({
      index,
      key,
      input: { ...input },
    }));
    if (jobs.length === 0) return false;
    await Promise.all(jobs.map(({ key, input }) =>
      this.markers.processEntryAsync(key, input)
    ));
    return generation === this.pageGeneration
      && JSON.stringify(jobs) === JSON.stringify(this.markerJobs);
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

  getToolTipsOverRange(
    entryKey: string,
    entryPart: EntryPart,
    start: number,
    end: number,
  ): string | null {
    return this.markers.getToolTipsOverRange(entryKey, entryPart, start, end);
  }

  getTooltipTextsOverRange(
    entryKey: string,
    entryPart: EntryPart,
    start: number,
    end: number,
  ): string[] {
    return this.markers.getTooltipTextsOverRange(entryKey, entryPart, start, end);
  }

  getLoadedRange(): { first: number; last: number } {
    return { first: this.firstLoaded, last: this.lastLoaded };
  }

  hasMoreBefore(): boolean {
    return this.visibleEntryIndices.indexOf(this.firstLoaded) > 0;
  }

  hasMoreAfter(): boolean {
    const last = this.visibleEntryIndices.indexOf(this.lastLoaded);
    return last >= 0 && last < this.visibleEntryIndices.length - 1;
  }

  captureScrollAnchor(
    viewportTop: number,
    candidates: readonly RendererScrollAnchorCandidate[],
  ): RendererScrollAnchor | null {
    const candidate =
      candidates.find(({ bottom }) => Number.isFinite(bottom) && bottom > viewportTop)
      ?? candidates.at(-1);
    if (!candidate || !Number.isFinite(candidate.top)) return null;
    return { key: candidate.key, offset: candidate.top - viewportTop };
  }

  scrollAdjustmentForAnchor(
    anchor: RendererScrollAnchor | null,
    viewportTop: number,
    candidates: readonly RendererScrollAnchorCandidate[],
  ): number {
    if (!anchor) return 0;
    const candidate = candidates.find(({ key }) => key === anchor.key);
    if (!candidate || !Number.isFinite(candidate.top)) return 0;
    return candidate.top - viewportTop - anchor.offset;
  }

  private updateMarkerJobs(jobs: readonly RendererMarkerJob[]): void {
    const previous = JSON.stringify(this.markerJobs);
    const next = jobs.map(({ index, key, input }) => ({
      index,
      key,
      input: { ...input },
    }));
    if (previous === JSON.stringify(next)) return;
    this.markerJobs = next;
    this.pageGeneration += 1;
    this.markers.retainEntries(next.map(({ key }) => key));
  }

  private markerInput(entry: RendererProjectionEntry, active: boolean): MarkerInput {
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

  private entryKey(index: number, entry: RendererProjectionEntry): string {
    return entry.key
      ? JSON.stringify(entry.key)
      : JSON.stringify({
          index,
          file: entry.file,
          source_text: entry.source,
          id: entry.id ?? null,
        });
  }
}
