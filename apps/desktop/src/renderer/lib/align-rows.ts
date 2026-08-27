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

export type AlignVisibleRows = {
  firstRow: number;
  lastRow: number;
};

export type AlignDragViewport = AlignVisibleRows & {
  pointerY: number;
  viewportTop: number;
  viewportBottom: number;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  rowCount: number;
  edgeSize?: number;
  maxStep?: number;
};

export type AlignDragViewportResult = {
  delta: number;
  focusRow: number | null;
};

/**
 * Model native JTable drag autoscroll for the renderer viewport.
 *
 * The focus row follows the nearest visible row while scrolling, then exposes
 * the explicit before-first/after-last drop boundary once the viewport reaches
 * either end. `delta` is clamped to the remaining scroll range so the renderer
 * never oscillates beyond a boundary.
 */
export function alignmentDragViewport(
  state: AlignDragViewport,
): AlignDragViewportResult {
  if (
    state.rowCount <= 0 ||
    !Number.isFinite(state.pointerY) ||
    state.viewportBottom <= state.viewportTop
  ) {
    return { delta: 0, focusRow: null };
  }
  const height = state.viewportBottom - state.viewportTop;
  const edgeSize = Math.max(1, Math.min(state.edgeSize ?? 48, height / 2));
  const maxStep = Math.max(1, Math.floor(state.maxStep ?? 24));
  const maxScrollTop = Math.max(0, state.scrollHeight - state.clientHeight);
  const scrollTop = Math.max(0, Math.min(state.scrollTop, maxScrollTop));
  const first = Math.max(0, Math.min(state.firstRow, state.rowCount - 1));
  const last = Math.max(first, Math.min(state.lastRow, state.rowCount - 1));

  const topDistance = state.pointerY - state.viewportTop;
  if (topDistance < edgeSize) {
    const pressure = 1 - Math.max(0, topDistance) / edgeSize;
    const requested = -Math.max(1, Math.ceil(maxStep * pressure));
    return {
      delta: Math.max(-scrollTop, requested),
      focusRow: scrollTop === 0 ? -1 : first,
    };
  }

  const bottomDistance = state.viewportBottom - state.pointerY;
  if (bottomDistance < edgeSize) {
    const pressure = 1 - Math.max(0, bottomDistance) / edgeSize;
    const requested = Math.max(1, Math.ceil(maxStep * pressure));
    return {
      delta: Math.min(maxScrollTop - scrollTop, requested),
      focusRow: scrollTop === maxScrollTop ? state.rowCount : last,
    };
  }

  return { delta: 0, focusRow: null };
}

/**
 * Return the selected edge that must be brought into the current viewport.
 * A range taller than the viewport follows its focus/lead row, like JTable.
 */
export function alignmentScrollTarget(
  visible: AlignVisibleRows,
  anchor: number,
  focus: number,
  rowCount: number,
): number | null {
  if (rowCount <= 0) return null;
  const bounds = selectionBounds(anchor, focus, rowCount);
  const first = Math.max(0, Math.min(visible.firstRow, rowCount - 1));
  const last = Math.max(first, Math.min(visible.lastRow, rowCount - 1));
  if (bounds.start >= first && bounds.end <= last) return null;
  if (bounds.start < first && bounds.end > last) {
    return Math.max(0, Math.min(focus, rowCount - 1));
  }
  return bounds.start < first ? bounds.start : bounds.end;
}

export function alignmentPointerSelection(
  current: { anchor: number; focus: number },
  row: number,
  rowCount: number,
  extend: boolean,
) {
  if (rowCount <= 0) return { anchor: 0, focus: 0 };
  const next = Math.max(0, Math.min(row, rowCount - 1));
  return {
    anchor: extend ? Math.max(0, Math.min(current.anchor, rowCount - 1)) : next,
    focus: next,
  };
}

export type AlignEditSelection = {
  anchor_row: number;
  focus_row: number;
};

export function alignmentSelectionAfterEdit(
  current: { anchor: number; focus: number },
  response: { row_count?: number; selection?: AlignEditSelection | null },
) {
  const rowCount = Math.max(0, response.row_count ?? 0);
  const last = Math.max(0, rowCount - 1);
  const clamp = (row: number) => Math.max(0, Math.min(row, last));
  const restored = response.selection;
  if (
    restored &&
    Number.isInteger(restored.anchor_row) &&
    Number.isInteger(restored.focus_row)
  ) {
    return {
      anchor: clamp(restored.anchor_row),
      focus: clamp(restored.focus_row),
    };
  }
  return {
    anchor: clamp(current.anchor),
    focus: clamp(current.focus),
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
    pageRows?: number;
  },
): AlignKeyboardResult {
  const base: AlignKeyboardResult = { ...state, handled: false };
  const last = Math.max(0, state.rowCount - 1);
  const pageRows = Math.max(1, Math.floor(input.pageRows ?? 10));
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
      return move(state.row - pageRows);
    case "PageDown":
      return move(state.row + pageRows);
    case "ArrowLeft":
      return {
        ...state,
        side: input.shiftKey && state.side === "target" ? "both" : "source",
        handled: true,
      };
    case "ArrowRight":
      return {
        ...state,
        side: input.shiftKey && state.side === "source" ? "both" : "target",
        handled: true,
      };
    case "Tab":
      return {
        ...state,
        side: input.shiftKey ? "source" : "target",
        handled: true,
      };
  }
  if (input.altKey || input.ctrlKey || input.metaKey) return base;
  const key = input.key.toLowerCase();
  // AlignPanelController installs Swing's Emacs-style N/P/F/B table actions.
  if (key === "n") return move(state.row + 1);
  if (key === "p") return move(state.row - 1);
  if (key === "f") {
    return {
      ...state,
      side: input.shiftKey
        ? state.side === "source"
          ? "both"
          : state.side
        : state.side === "both"
          ? "source"
          : "target",
      handled: true,
    };
  }
  if (key === "b") {
    return {
      ...state,
      side: input.shiftKey
        ? state.side === "target"
          ? "both"
          : state.side
        : state.side === "both"
          ? "target"
          : "source",
      handled: true,
    };
  }
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
