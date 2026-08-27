/** Java `org.omegat.gui.editor.MarkerController`. */
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { calcMarkers } from "./mark/CalcMarkersThread";
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
};

export class MarkerController {
  markers: IMarker[] = [
    new WhitespaceMarker(),
    new NBSPMarker(),
    new BidiMarkers(),
    new ProtectedPartsMarker(),
    new AltTranslationsMarker(),
    new ComesFromAutoTMMarker(),
    new ComesFromMTMarker(),
    new FontFallbackMarker(),
    new RemoveTagMarker(),
    new ReplaceMarker(),
  ];
  private generation = 0;
  private readonly cache = new Map<string, CachedMarkers>();

  process(input: MarkerInput): Mark[] {
    return calcMarkers(this.markers, input);
  }

  processEntry(entryKey: string, input: MarkerInput): MarkerSnapshot {
    const fingerprint = JSON.stringify(input);
    const cached = this.cache.get(entryKey);
    if (cached?.fingerprint === fingerprint) {
      return {
        entryKey,
        generation: cached.generation,
        marks: cached.marks.map((mark) => ({ ...mark })),
      };
    }
    const snapshot: CachedMarkers = {
      entryKey,
      generation: ++this.generation,
      marks: this.process(input),
      fingerprint,
    };
    this.cache.set(entryKey, snapshot);
    return {
      entryKey,
      generation: snapshot.generation,
      marks: snapshot.marks.map((mark) => ({ ...mark })),
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

  invalidate(entryKey?: string): void {
    if (entryKey === undefined) {
      this.cache.clear();
    } else {
      this.cache.delete(entryKey);
    }
  }
}
