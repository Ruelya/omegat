/** Java `org.omegat.gui.editor.mark.SymbolPainter`. */
export class SymbolPainter {
  constructor(public color: string, public symbol: string) {}
  paint(): string {
    return this.symbol;
  }
}
