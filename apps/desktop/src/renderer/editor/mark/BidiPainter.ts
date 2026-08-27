/** Java `org.omegat.gui.editor.mark.BidiPainter`. */
export function paintBidi(ch: string): string {
  return `bidi:${ch.codePointAt(0)?.toString(16) ?? "?"}`;
}
