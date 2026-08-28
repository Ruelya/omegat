/** Java `org.omegat.gui.editor.filter.ReplaceFilter`. */
export type ReplaceFilter = { needle: string; replacement: string };

export function applyReplace(text: string, f: ReplaceFilter): string {
  if (!f.needle) return text;
  return text.split(f.needle).join(f.replacement);
}
