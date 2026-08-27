/** Java `org.omegat.gui.editor.ViewLabel`. */
export function viewLabel(n: number, source: boolean): string {
  return source ? `${n} ›` : `${n} <`;
}
