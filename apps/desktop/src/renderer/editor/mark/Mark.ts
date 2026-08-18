/** Java `org.omegat.gui.editor.mark.Mark`. */
export type EntryPart = "SOURCE" | "TRANSLATION";

export type Mark = {
  startOffset: number;
  endOffset: number;
  painter: string;
  toolTipText?: string;
  entryPart: EntryPart;
};

export function mark(
  start: number,
  end: number,
  painter: string,
  toolTipText?: string,
  source = false,
): Mark {
  return {
    startOffset: start,
    endOffset: end,
    painter,
    toolTipText,
    entryPart: source ? "SOURCE" : "TRANSLATION",
  };
}
