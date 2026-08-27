/** Java `org.omegat.gui.editor.mark.FontFallbackMarker` — Font.canDisplay, not a code-point range. */
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

let measureCtx: CanvasRenderingContext2D | null | undefined;

function canvasContext(): CanvasRenderingContext2D | null {
  if (measureCtx !== undefined) return measureCtx;
  if (typeof document === "undefined") {
    measureCtx = null;
    return null;
  }
  const canvas = document.createElement("canvas");
  measureCtx = canvas.getContext("2d");
  return measureCtx;
}

/** Probe whether the UI font can draw `ch` (Java `Font.canDisplay`). */
export function fontCanDisplay(ch: string, font = "16px sans-serif"): boolean {
  const ctx = canvasContext();
  if (!ctx) {
    const cp = ch.codePointAt(0) ?? 0;
    return cp <= 0x024f || cp >= 0x1f000;
  }
  ctx.font = font;
  const w = ctx.measureText(ch).width;
  const tofu = ctx.measureText("\ufffd").width;
  const space = ctx.measureText(" ").width;
  if (w === 0) return false;
  if (ch === "\ufffd") return true;
  return w !== tofu || ch === "\ufffd" || w !== space;
}

export class FontFallbackMarker extends AbstractMarker {
  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.isEnabled()) return null;
    const text = input.translationText ?? input.sourceText;
    if (text == null) return null;
    const out: Mark[] = [];
    let index = 0;
    for (const char of text) {
      if (!fontCanDisplay(char)) out.push(mark(index, index + char.length, "font-fallback"));
      index += char.length;
    }
    return out;
  }
}

export function fontFallbackMarker(text: string) {
  return new FontFallbackMarker().getMarksForEntry({ sourceText: text, translationText: text, isActive: true }) ?? [];
}
