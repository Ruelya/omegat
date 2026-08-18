/** Java `org.omegat.gui.editor.mark.FontFallbackMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class FontFallbackMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText ?? input.sourceText;
    const out: Mark[] = [];
    for (let i = 0; i < text.length; i++) {
      const u = text.charCodeAt(i);
      if (u > 0x024f && u < 0x1f000) out.push(mark(i, i + 1, "font-fallback"));
    }
    return out;
  }
}

export function fontFallbackMarker(text: string) {
  return new FontFallbackMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
