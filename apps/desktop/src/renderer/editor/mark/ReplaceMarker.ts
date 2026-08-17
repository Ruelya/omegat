/** Java `org.omegat.gui.editor.mark.ReplaceMarker`. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ReplaceMarker extends AbstractMarker {
  needle = "";
  replacement = "";
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled() || !this.needle) return null;
    const text = input.translationText || input.sourceText;
    const out: Mark[] = [];
    let i = 0;
    while (i < text.length) {
      const at = text.indexOf(this.needle, i);
      if (at < 0) break;
      out.push(mark(at, at + this.needle.length, "replace", this.replacement));
      i = at + this.needle.length;
    }
    return out;
  }
}

export function replaceMarker(text: string, needle: string, replacement: string) {
  const m = new ReplaceMarker();
  m.needle = needle;
  m.replacement = replacement;
  return m.getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
