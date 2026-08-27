export type AlignBead = {
  source: string;
  target: string;
  source_lines: (string | null)[];
  target_lines: (string | null)[];
  score: number;
  enabled: boolean;
  status: "default" | "accepted" | "needs-review";
};

export type AlignSide = "both" | "source" | "target";

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

export type AlignTableDrop = {
  startRow: number;
  endRow: number;
  side: Exclude<AlignSide, "both">;
  targetRow: number;
  targetSide: Exclude<AlignSide, "both">;
};

export type AlignTableDropResult = {
  allowed: boolean;
  action?: "move-to-row";
  extra?: {
    start_row: number;
    end_row: number;
    side: Exclude<AlignSide, "both">;
    target_row: number;
  };
};

/**
 * Apply Java `AlignTransferHandler.canImport` rules before issuing a drop.
 * Empty cells do not transfer, the drop column must match, and only the
 * leading/trailing line of a bead can cross into another bead.
 */
export function alignTableDrop(
  beads: AlignBead[],
  drop: AlignTableDrop,
): AlignTableDropResult {
  if (drop.side !== drop.targetSide) return { allowed: false };
  const rows = alignmentRows(beads);
  if (!rows.length) return { allowed: false };
  const bounds = selectionBounds(drop.startRow, drop.endRow, rows.length);
  const realRows = rows
    .slice(bounds.start, bounds.end + 1)
    .filter((row) => (drop.side === "source" ? row.source : row.target) != null);
  const first = realRows[0];
  const last = realRows.at(-1);
  if (!first || !last) return { allowed: false };
  const movingUp = drop.targetRow < first.rowIndex;
  const movingDown = drop.targetRow > last.rowIndex;
  if (!movingUp && !movingDown) return { allowed: false };
  const boundary = movingUp ? first : last;
  const bead = beads[boundary.beadIndex];
  const lineIndex =
    drop.side === "source" ? boundary.sourceLineIndex : boundary.targetLineIndex;
  if (lineIndex == null) return { allowed: false };
  const sideLines = drop.side === "source" ? bead.source_lines : bead.target_lines;
  const oppositeLines =
    drop.side === "source" ? bead.target_lines : bead.source_lines;
  const atTableBoundary =
    (movingUp && boundary.rowIndex === 0) ||
    (movingDown && boundary.rowIndex === rows.length - 1);
  const movable = atTableBoundary
    ? oppositeLines.length > 0
    : movingUp
      ? lineIndex === 0
      : lineIndex === sideLines.length - 1;
  if (!movable) return { allowed: false };
  const target = rows[drop.targetRow];
  if (target && target.beadIndex === boundary.beadIndex) {
    return { allowed: false };
  }
  return {
    allowed: true,
    action: "move-to-row",
    extra: {
      start_row: bounds.start,
      end_row: bounds.end,
      side: drop.side,
      target_row: drop.targetRow,
    },
  };
}

export type AlignPinpoint = {
  row: number;
  side: Exclude<AlignSide, "both">;
};

export type AlignKeyboardState = {
  row: number;
  anchor: number;
  rowCount: number;
  side: AlignSide;
  pinpoint: AlignPinpoint | null;
};

export type AlignKeyboardResult = AlignKeyboardState & {
  handled: boolean;
  action?: string;
  extra?: Record<string, unknown>;
  focusEditor?: boolean;
};

/**
 * Product keyboard model for the manual-alignment table.
 *
 * The unmodified U/D/S/M/E/A/R/C/K/Space/Escape accelerators mirror
 * `AlignMenuFrame`; navigation retains a Swing-like anchor/focus row range.
 * Pinpoint completion is destructive only when both its row and column differ,
 * matching `AlignPanelController.pinpointAlign`.
 */
export function alignTableKey(
  state: AlignKeyboardState,
  input: {
    key: string;
    shiftKey?: boolean;
    altKey?: boolean;
    ctrlKey?: boolean;
    metaKey?: boolean;
  },
): AlignKeyboardResult {
  const base: AlignKeyboardResult = { ...state, handled: false };
  const last = Math.max(0, state.rowCount - 1);
  const move = (row: number): AlignKeyboardResult => {
    const next = Math.max(0, Math.min(row, last));
    return {
      ...state,
      row: next,
      anchor: input.shiftKey ? state.anchor : next,
      handled: true,
    };
  };
  switch (input.key) {
    case "ArrowUp":
      return move(state.row - 1);
    case "ArrowDown":
      return move(state.row + 1);
    case "Home":
      return move(0);
    case "End":
      return move(last);
    case "PageUp":
      return move(state.row - 10);
    case "PageDown":
      return move(state.row + 10);
    case "ArrowLeft":
      return { ...state, side: "source", handled: true };
    case "ArrowRight":
      return { ...state, side: "target", handled: true };
    case "Tab":
      return {
        ...state,
        side: input.shiftKey ? "source" : "target",
        handled: true,
      };
  }
  if (input.altKey || input.ctrlKey || input.metaKey) return base;
  const key = input.key.toLowerCase();
  if (key === "escape") {
    return { ...state, pinpoint: null, handled: state.pinpoint != null };
  }
  if (key === " ") {
    if (state.side === "both" || state.rowCount === 0) return base;
    if (state.pinpoint == null) {
      return {
        ...state,
        pinpoint: { row: state.row, side: state.side },
        handled: true,
      };
    }
    if (state.pinpoint.row === state.row || state.pinpoint.side === state.side) {
      return { ...state, handled: true };
    }
    return {
      ...state,
      pinpoint: null,
      handled: true,
      action: "pinpoint",
      extra: {
        start_row: state.pinpoint.row,
        end_row: state.row,
        side: state.pinpoint.side,
        end_side: state.side,
      },
    };
  }
  if (key === "e" || key === "enter") {
    return state.side === "both" || state.rowCount === 0
      ? base
      : { ...state, handled: true, focusEditor: true };
  }
  const action = (
    {
      u: "up",
      d: "down",
      s: "split",
      m: "merge",
      a: "accepted",
      r: "needs-review",
      c: "clear-status",
      k: "toggle-keep",
    } as Record<string, string>
  )[key];
  if (!action || state.rowCount === 0) return base;
  if (["up", "down", "split", "merge"].includes(action) && state.side === "both") {
    return base;
  }
  return { ...state, handled: true, action };
}
