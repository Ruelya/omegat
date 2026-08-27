export type AlignBead = {
  source: string;
  target: string;
  source_lines: (string | null)[];
  target_lines: (string | null)[];
  score: number;
  enabled: boolean;
  status: "default" | "accepted" | "needs-review";
};

export type AlignRow = {
  rowIndex: number;
  beadIndex: number;
  rowInBead: number;
  sourceLineIndex: number | null;
  targetLineIndex: number | null;
  source: string | null;
  target: string | null;
};

export function alignmentRows(beads: AlignBead[]): AlignRow[] {
  const rows: AlignRow[] = [];
  beads.forEach((bead, beadIndex) => {
    const count = Math.max(bead.source_lines.length, bead.target_lines.length);
    for (let rowInBead = 0; rowInBead < count; rowInBead += 1) {
      const sourceLineIndex = rowInBead < bead.source_lines.length ? rowInBead : null;
      const targetLineIndex = rowInBead < bead.target_lines.length ? rowInBead : null;
      rows.push({
        rowIndex: rows.length,
        beadIndex,
        rowInBead,
        sourceLineIndex,
        targetLineIndex,
        source: sourceLineIndex == null ? null : bead.source_lines[sourceLineIndex] ?? null,
        target: targetLineIndex == null ? null : bead.target_lines[targetLineIndex] ?? null,
      });
    }
  });
  return rows;
}

export function selectionBounds(anchor: number, focus: number, rowCount: number) {
  const last = Math.max(0, rowCount - 1);
  return {
    start: Math.min(anchor, focus, last),
    end: Math.min(Math.max(anchor, focus), last),
  };
}
