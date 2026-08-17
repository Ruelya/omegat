/** Java `org.omegat.gui.editor.mark.RemoveTagMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;

export class RemoveTagMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const src = new Set(input.sourceText.match(TAG) ?? []);
    const out: Mark[] = [];
    for (const m of input.translationText.matchAll(TAG)) {
      if (!src.has(m[0])) out.push(mark(m.index ?? 0, (m.index ?? 0) + m[0].length, "remove-tag", m[0]));
    }
    return out;
  }
}

export function removeTagMarker(source: string, target: string) {
  return new RemoveTagMarker().getMarksForEntry({ sourceText: source, translationText: target, isActive: true }) ?? [];
}
