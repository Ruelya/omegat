/** Java `org.omegat.gui.editor.filter.SearchFilter`. */
export type SearchFilter = { query: string; regex: boolean };

export function searchFilterMatches(source: string, translation: string, f: SearchFilter): boolean {
  if (!f.query) return true;
  const hay = `${source}\n${translation}`;
  if (f.regex) {
    try {
      return new RegExp(f.query, "i").test(hay);
    } catch {
      return hay.toLowerCase().includes(f.query.toLowerCase());
    }
  }
  return hay.toLowerCase().includes(f.query.toLowerCase());
}
