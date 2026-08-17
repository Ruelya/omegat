/** Java `org.omegat.gui.editor.history.WordCompleter`. */
export function completeWords(translations: string[], prefix: string): string[] {
  if (!prefix) return [];
  const p = prefix.toLowerCase();
  const seen = new Set<string>();
  const out: string[] = [];
  for (const text of translations) {
    for (const w of text.split(/[^\p{L}\p{N}']+/u)) {
      if (w.length > 1 && w.toLowerCase().startsWith(p) && w.toLowerCase() !== p && !seen.has(w)) {
        seen.add(w);
        out.push(w);
      }
    }
  }
  return out;
}
