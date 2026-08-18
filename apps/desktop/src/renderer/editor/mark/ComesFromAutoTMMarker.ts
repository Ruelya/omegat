/** Java `org.omegat.gui.editor.mark.ComesFromAutoTMMarker`. */
import { TransparentHighlightPainter } from "./TransparentHighlightPainter";
import { colorForLinked, type LinkedTm } from "./EditorColor";
import { AbstractMarker } from "./AbstractMarker";
import { mark, type Mark } from "./Mark";
import type { MarkerInput } from "./IMarker";

export class ComesFromAutoTMMarker extends AbstractMarker {
  markAutoPopulated = true;
  linked: LinkedTm = "xAUTO";

  getMarksForEntry(input: MarkerInput): Mark[] | null {
    if (!this.markAutoPopulated) return null;
    if (input.sourceText == null && input.translationText == null) return null;
    if (!input.fromAuto || input.translationText == null) return null;
    const color = colorForLinked(input.linked ?? this.linked);
    // Painter is created per call so color preference changes take effect.
    const painter = new TransparentHighlightPainter(color.getColor(), 0.5);
    const m = mark(0, input.translationText.length, "auto-tm", "AUTO_TM");
    m.painterColor = painter.color;
    return [m];
  }
}

export function comesFromAutoTMMarker(fromAuto: boolean, len: number) {
  return (
    new ComesFromAutoTMMarker().getMarksForEntry({
      sourceText: fromAuto ? "Edit" : "",
      translationText: fromAuto ? "x".repeat(len) : null,
      isActive: true,
      fromAuto,
    }) ?? []
  );
}
