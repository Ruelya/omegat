/** Java `org.omegat.gui.editor.MarkerController`. */
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { ComesFromMTMarker } from "./mark/ComesFromMTMarker";
import { FontFallbackMarker } from "./mark/FontFallbackMarker";
import type { IMarker, MarkerInput } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";
import type { Document3State, StyledSpan } from "./Document3";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { RemoveTagMarker } from "./mark/RemoveTagMarker";
import { ReplaceMarker } from "./mark/ReplaceMarker";
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
  marker: IMarker;
  plugin: boolean;
  registration: number;
};

export class MarkerController {
  private readonly registered: RegisteredMarker[];
  private generation = 0;
  private registration = 0;
  private readonly cache = new Map<string, CachedMarkers>();

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
  }

  get markers(): IMarker[] {
    return this.registered.map(({ marker }) => marker);
  }

  getMarkerNames(): string[] {
    return this.registered.map(({ name }) => name);
  }

  registerPluginMarker(name: string, marker: IMarker): void {
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
    return this.registered.flatMap(({ marker }) =>
      marker.getMarksForEntry(input)?.map((mark) => ({ ...mark })) ?? [],
    );
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

  applyToDocument(
    entryKey: string,
    document: Document3State,
    input: MarkerInput,
  ): { document: Document3State; snapshot: MarkerSnapshot } {
    const snapshot = this.processEntry(entryKey, input);
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
      document: {
        ...document,
        spans: [
          ...document.spans.filter((span) => !span.style.startsWith("marker:")),
          ...markerSpans,
        ],
      },
      snapshot,
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
}
