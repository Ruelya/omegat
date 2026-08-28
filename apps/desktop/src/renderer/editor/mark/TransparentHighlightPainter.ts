/** Java `org.omegat.gui.editor.mark.TransparentHighlightPainter`. */
export class TransparentHighlightPainter {
  constructor(public color: string, public alpha = 0.35) {}
  css(): string {
    return `color-mix(in srgb, ${this.color} ${Math.round(this.alpha * 100)}%, transparent)`;
  }
}
