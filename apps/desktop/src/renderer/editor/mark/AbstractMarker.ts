/** Java `org.omegat.gui.editor.mark.AbstractMarker`. */
import type { IMarker, MarkerInput } from "./IMarker";
import { mark, type Mark } from "./Mark";

export abstract class AbstractMarker implements IMarker {
  enabled = true;
  pattern: RegExp | null = null;
  toolTip = "";
  painter = "";

  isEnabled(): boolean {
    return this.enabled;
  }

  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    if (!this.pattern) return [];
    const out: Mark[] = [];
    const markSource = input.isActive || input.displaySource || input.translationText == null;
    if (markSource && input.sourceText != null) {
      this.collect(input.sourceText, true, out);
    }
    if (input.translationText != null) {
      this.collect(input.translationText, false, out);
    }
    return out;
  }

  protected collect(text: string, source: boolean, out: Mark[]): void {
    if (!this.pattern) return;
    const re = new RegExp(this.pattern.source, this.pattern.flags.includes("g") ? this.pattern.flags : `${this.pattern.flags}g`);
    for (const m of text.matchAll(re)) {
      const start = m.index ?? 0;
      out.push(mark(start, start + m[0].length, this.painter, this.toolTip, source));
    }
  }
}
