/** Java `org.omegat.gui.editor.filter.FilterBarSearch`. */
import { searchFilterMatches, type SearchFilter } from "./SearchFilter";

export function filterBarSearch(entries: { source: string; translation: string }[], f: SearchFilter) {
  return entries.filter((e) => searchFilterMatches(e.source, e.translation, f));
}
