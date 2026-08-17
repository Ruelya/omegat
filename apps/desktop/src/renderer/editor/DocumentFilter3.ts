/** Java `org.omegat.gui.editor.DocumentFilter3` — tag atoms are not split. */
const TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;

export function isInsideTag(text: string, offset: number): boolean {
  for (const m of text.matchAll(TAG)) {
    const s = m.index ?? 0;
    if (offset > s && offset < s + m[0].length) return true;
  }
  return false;
}

export function allowInsert(text: string, offset: number): boolean {
  return !isInsideTag(text, offset);
}
