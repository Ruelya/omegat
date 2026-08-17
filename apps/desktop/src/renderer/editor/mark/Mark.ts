/** Java `org.omegat.gui.editor.mark.Mark`. */
export type Mark = {
  startOffset: number;
  endOffset: number;
  painter: string;
  toolTipText?: string;
  entryPartSource: boolean;
};

export function mark(start: number, end: number, painter: string, toolTipText?: string, source = false): Mark {
  return { startOffset: start, endOffset: end, painter, toolTipText, entryPartSource: source };
}
