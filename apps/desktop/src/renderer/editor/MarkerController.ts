/** Java `org.omegat.gui.editor.MarkerController`. */
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { ComesFromMTMarker } from "./mark/ComesFromMTMarker";
import { FontFallbackMarker } from "./mark/FontFallbackMarker";
import {
  isAsyncMarker,
  type IMarker,
  type MarkerInput,
  type MarkerProvider,
} from "./mark/IMarker";
import type { Mark } from "./mark/Mark";
import type { Document3State, StyledSpan } from "./Document3";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { RemoveTagMarker } from "./mark/RemoveTagMarker";
import { ReplaceMarker } from "./mark/ReplaceMarker";
import { SpellCheckerMarker } from "./mark/SpellCheckerMarker";
import { WhitespaceMarker } from "./mark/WhitespaceMarker";

export type MarkerSnapshot = {
  entryKey: string;
  generation: number;
  marks: Mark[];
};

type CachedMarkers = MarkerSnapshot & {
  fingerprint: string;
  byMarker: Map<string, {
    registration: number;
    marks: Mark[];
  }>;
};

type RegisteredMarker = {
  name: string;
  marker: MarkerProvider;
  plugin: boolean;
  registration: number;
};

export class MarkerController {
  private readonly registered: RegisteredMarker[];
  private generation = 0;
  private registration = 0;
  private request = 0;
  private readonly cache = new Map<string, CachedMarkers>();
  private readonly pending = new Map<string, number>();

  constructor() {
    const builtins: [string, IMarker][] = [
      ["WhitespaceMarker", new WhitespaceMarker()],
      ["NBSPMarker", new NBSPMarker()],
      ["BidiMarkers", new BidiMarkers()],
      ["ProtectedPartsMarker", new ProtectedPartsMarker()],
      ["AltTranslationsMarker", new AltTranslationsMarker()],
      ["ComesFromAutoTMMarker", new ComesFromAutoTMMarker()],
      ["ComesFromMTMarker", new ComesFromMTMarker()],
      ["FontFallbackMarker", new FontFallbackMarker()],
      ["RemoveTagMarker", new RemoveTagMarker()],
      ["ReplaceMarker", new ReplaceMarker()],
    ];
    this.registered = builtins.map(([simpleName, marker]) => ({
      name: `org.omegat.gui.editor.mark.${simpleName}`,
      marker,
      plugin: false,
      registration: ++this.registration,
    }));
    this.registered.push({
      name: "org.omegat.core.spellchecker.SpellCheckerMarker",
      marker: new SpellCheckerMarker(),
      plugin: false,
      registration: ++this.registration,
    });
  }

  get markers(): MarkerProvider[] {
    return this.registered.map(({ marker }) => marker);
  }

  getMarkerNames(): string[] {
    return this.registered.map(({ name }) => name);
  }

  registerPluginMarker(name: string, marker: MarkerProvider): void {
    const normalized = name.trim();
    if (!normalized) throw new Error("marker name is required");
    if (this.resolveMarkerName(normalized)) {
      throw new Error(`marker already registered: ${normalized}`);
    }
    this.registered.push({
      name: normalized,
      marker,
      plugin: true,
      registration: ++this.registration,
    });
    this.invalidate();
  }

  unregisterPluginMarker(name: string): boolean {
    const resolved = this.resolveMarkerName(name);
    if (!resolved) return false;
    const index = this.registered.findIndex(
      (registration) => registration.name === resolved && registration.plugin,
    );
    if (index < 0) return false;
    this.registered.splice(index, 1);
    this.invalidate();
    return true;
  }

  process(input: MarkerInput): Mark[] {
    return this.registered.flatMap(({ marker }) => {
      if (isAsyncMarker(marker)) return [];
      return marker.getMarksForEntry(input)?.map((mark) => ({ ...mark })) ?? [];
    });
  }

  processEntry(entryKey: string, input: MarkerInput): MarkerSnapshot {
    const fingerprint = JSON.stringify(input);
    const previous = this.cache.get(entryKey);
    const cached: CachedMarkers = previous?.fingerprint === fingerprint
      ? previous
      : {
          entryKey,
          generation: 0,
          marks: [],
          fingerprint,
          byMarker: new Map(),
        };
    let changed = cached !== previous;
    const liveNames = new Set(this.registered.map(({ name }) => name));
    for (const name of cached.byMarker.keys()) {
      if (!liveNames.has(name)) {
        cached.byMarker.delete(name);
        changed = true;
      }
    }
    for (const registration of this.registered) {
      if (isAsyncMarker(registration.marker)) continue;
      const prior = cached.byMarker.get(registration.name);
      if (prior?.registration === registration.registration) continue;
      cached.byMarker.set(registration.name, {
        registration: registration.registration,
        marks:
          registration.marker
            .getMarksForEntry(input)
            ?.map((mark) => ({ ...mark })) ?? [],
      });
      changed = true;
    }
    if (changed) {
      cached.generation = ++this.generation;
      cached.marks = this.registered.flatMap(({ name }) =>
        cached.byMarker.get(name)?.marks.map((mark) => ({ ...mark })) ?? [],
      );
    }
    this.cache.set(entryKey, cached);
    return {
      entryKey,
      generation: cached.generation,
      marks: cached.marks.map((mark) => ({ ...mark })),
    };
  }

  /**
   * Calculate asynchronous marker providers with a per-entry/per-marker token.
   * A later calculation, edit, `remarkOneMarker`, or unload invalidates the
   * token, so a slow callback can never publish marks for an older document.
   */
  async processEntryAsync(entryKey: string, input: MarkerInput): Promise<MarkerSnapshot> {
    this.processEntry(entryKey, input);
    const fingerprint = JSON.stringify(input);
    const jobs = this.registered.flatMap((registration) => {
      if (!isAsyncMarker(registration.marker)) return [];
      const cache = this.cache.get(entryKey);
      const prior = cache?.byMarker.get(registration.name);
      if (cache?.fingerprint === fingerprint && prior?.registration === registration.registration) {
        return [];
      }
      const pendingKey = this.pendingKey(entryKey, registration.name);
      const token = ++this.request;
      this.pending.set(pendingKey, token);
      return [{
        registration,
        pendingKey,
        token,
        result: registration.marker.getMarksForEntryAsync(input),
      }];
    });

    await Promise.all(jobs.map(async ({ registration, pendingKey, token, result }) => {
      let marks: Mark[] | null;
      try {
        marks = await result;
      } catch {
        if (this.pending.get(pendingKey) === token) {
          this.pending.delete(pendingKey);
        }
        return;
      }
      if (this.pending.get(pendingKey) !== token) return;
      const live = this.registered.find(({ name }) => name === registration.name);
      const cache = this.cache.get(entryKey);
      if (
        live?.registration !== registration.registration
        || cache?.fingerprint !== fingerprint
      ) {
        return;
      }
      cache.byMarker.set(registration.name, {
        registration: registration.registration,
        marks: marks?.map((mark) => ({ ...mark })) ?? [],
      });
      cache.generation = ++this.generation;
      cache.marks = this.registered.flatMap(({ name }) =>
        cache.byMarker.get(name)?.marks.map((mark) => ({ ...mark })) ?? [],
      );
      this.pending.delete(pendingKey);
    }));

    return this.getCached(entryKey) ?? {
      entryKey,
      generation: 0,
      marks: [],
    };
  }

  applyToDocument(
    entryKey: string,
    document: Document3State,
    input: MarkerInput,
  ): { document: Document3State; snapshot: MarkerSnapshot } {
    const snapshot = this.processEntry(entryKey, input);
    return {
      document: this.applySnapshotToDocument(document, snapshot),
      snapshot,
    };
  }

  async applyToDocumentAsync(
    entryKey: string,
    document: Document3State,
    input: MarkerInput,
  ): Promise<{ document: Document3State; snapshot: MarkerSnapshot }> {
    const snapshot = await this.processEntryAsync(entryKey, input);
    return {
      document: this.applySnapshotToDocument(document, snapshot),
      snapshot,
    };
  }

  private applySnapshotToDocument(
    document: Document3State,
    snapshot: MarkerSnapshot,
  ): Document3State {
    const sourceStart = document.fullText.indexOf(document.source);
    const markerSpans = snapshot.marks.flatMap((mark): StyledSpan[] => {
      const base = mark.entryPart === "TRANSLATION" ? document.translationStart : sourceStart;
      const limit =
        mark.entryPart === "TRANSLATION" ? document.translation.length : document.source.length;
      if (
        base < 0 ||
        mark.startOffset < 0 ||
        mark.endOffset <= mark.startOffset ||
        mark.endOffset > limit
      ) {
        return [];
      }
      return [{
        start: base + mark.startOffset,
        end: base + mark.endOffset,
        style: `marker:${mark.painter}${mark.painterColor ? `:${mark.painterColor}` : ""}`,
      }];
    });
    return {
      ...document,
      spans: [
        ...document.spans.filter((span) => !span.style.startsWith("marker:")),
        ...markerSpans,
      ],
    };
  }

  getCached(entryKey: string): MarkerSnapshot | null {
    const cached = this.cache.get(entryKey);
    return cached
      ? {
          entryKey,
          generation: cached.generation,
          marks: cached.marks.map((mark) => ({ ...mark })),
        }
      : null;
  }

  /**
   * Invalidate all marker output, one entry, or one marker across cached
   * entries. The latter is Java's `remarkOneMarker` lifecycle.
   */
  invalidate(entryKey?: string, markerName?: string): void {
    this.cancelPending(entryKey, markerName);
    const resolved = markerName ? this.resolveMarkerName(markerName) : null;
    if (markerName && !resolved) return;
    if (resolved) {
      const entries = entryKey === undefined
        ? [...this.cache.values()]
        : [this.cache.get(entryKey)].filter((entry): entry is CachedMarkers => Boolean(entry));
      for (const entry of entries) {
        entry.byMarker.delete(resolved);
      }
    } else if (entryKey === undefined) {
      this.cache.clear();
    } else {
      this.cache.delete(entryKey);
    }
  }

  remarkOneMarker(markerName: string, entryKey?: string): void {
    this.invalidate(entryKey, markerName);
  }

  private resolveMarkerName(name: string): string | null {
    const exact = this.registered.find(({ name: registered }) => registered === name);
    if (exact) return exact.name;
    const simple = this.registered.find(({ name: registered }) =>
      registered.endsWith(`.${name}`),
    );
    return simple?.name ?? null;
  }

  private pendingKey(entryKey: string, markerName: string): string {
    return `${entryKey}\u0000${markerName}`;
  }

  private cancelPending(entryKey?: string, markerName?: string): void {
    const resolved = markerName ? this.resolveMarkerName(markerName) : null;
    for (const key of this.pending.keys()) {
      const split = key.lastIndexOf("\u0000");
      const pendingEntry = key.slice(0, split);
      const pendingMarker = key.slice(split + 1);
      if (
        (entryKey === undefined || entryKey === pendingEntry)
        && (markerName === undefined || resolved === pendingMarker)
      ) {
        this.pending.delete(key);
      }
    }
  }
}
