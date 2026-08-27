/** Java `org.omegat.gui.editor.chartable.CharTableRenderer`. */
export function renderChar(ch: string): string {
  if (!ch) return "";
  return `${ch} U+${ch.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`;
}
