/** Java `org.omegat.gui.editor.mark.BidiMarkers`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const LRM = 0x200e;
const RLM = 0x200f;
const LRE = 0x202a;
const RLE = 0x202b;
const PDF = 0x202c;
const LRO = 0x202d;
const RLO = 0x202e;

const EMBED = new Set([LRE, RLE, LRO, RLO]);

function painterFor(cp: number): string {
  if (cp === LRE) return "lre";
  if (cp === RLE) return "rle";
  if (cp === LRO) return "lro";
  if (cp === RLO) return "rlo";
  if (cp === LRM) return "lrm";
  if (cp === RLM) return "rlm";
  return "bidi";
}

export class BidiMarkers extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText;
    if (!input.isActive || text == null || text.trim() === "") return [];
    const out: Mark[] = [];
    let startPos = -1;
    let markCp = -1;
    for (let i = 0; i < text.length; i++) {
      const cp = text.charCodeAt(i);
      if (![LRE, RLE, LRM, RLM, PDF, LRO, RLO].includes(cp)) continue;
      if (cp === PDF && startPos !== -1) {
        out.push(mark(startPos, i, painterFor(markCp), "BIDI", false));
        startPos = -1;
        markCp = -1;
      } else if (cp === LRM || cp === RLM) {
        out.push(mark(i, i + 1, painterFor(cp), "BIDI", false));
      } else if (EMBED.has(cp)) {
        markCp = cp;
        startPos = i;
      }
    }
    if (startPos !== -1) {
      out.push(mark(startPos, startPos, painterFor(markCp), "BIDI", false));
    }
    return out;
  }
}

export function bidiMarkers(text: string): Mark[] {
  return (
    new BidiMarkers().getMarksForEntry({
      sourceText: text,
      translationText: text,
      isActive: true,
    }) ?? []
  );
}
