/** Java `org.omegat.gui.editor.ViewParagraph`. */
export type ViewParagraph = { start: number; end: number; text: string };

export function paragraphs(text: string): ViewParagraph[] {
  const out: ViewParagraph[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i === text.length || text[i] === "\n") {
      out.push({ start, end: i, text: text.slice(start, i) });
      start = i + 1;
    }
  }
  return out;
}
