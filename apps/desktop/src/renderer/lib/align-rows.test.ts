import { describe, expect, it } from "vitest";
import {
  alignmentRows,
  alignTableDrop,
  alignTableKey,
  selectionBounds,
  type AlignBead,
  type AlignKeyboardState,
} from "./align-rows";

describe("alignment visual rows", () => {
  it("retains bead and nullable side indexes across a complete row span", () => {
    const beads: AlignBead[] = [
      {
        source: "one two",
        target: "un",
        source_lines: ["one", "two"],
        target_lines: ["un"],
        score: 1,
        enabled: true,
        status: "default",
      },
      {
        source: "three",
        target: "trois mots",
        source_lines: ["three"],
        target_lines: ["trois", "mots"],
        score: 2,
        enabled: true,
        status: "needs-review",
      },
    ];

    expect(alignmentRows(beads)).toEqual([
      {
        rowIndex: 0,
        beadIndex: 0,
        rowInBead: 0,
        sourceLineIndex: 0,
        targetLineIndex: 0,
        source: "one",
        target: "un",
      },
      {
        rowIndex: 1,
        beadIndex: 0,
        rowInBead: 1,
        sourceLineIndex: 1,
        targetLineIndex: null,
        source: "two",
        target: null,
      },
      {
        rowIndex: 2,
        beadIndex: 1,
        rowInBead: 0,
        sourceLineIndex: 0,
        targetLineIndex: 0,
        source: "three",
        target: "trois",
      },
      {
        rowIndex: 3,
        beadIndex: 1,
        rowInBead: 1,
        sourceLineIndex: null,
        targetLineIndex: 1,
        source: null,
        target: "mots",
      },
    ]);
    expect(selectionBounds(3, 1, 4)).toEqual({ start: 1, end: 3 });
  });

  it("models Swing-style navigation, accelerators, and pinpoint constraints", () => {
    const state: AlignKeyboardState = {
      row: 1,
      anchor: 1,
      rowCount: 4,
      side: "source",
      pinpoint: null,
    };
    expect(alignTableKey(state, { key: "ArrowDown", shiftKey: true })).toEqual({
      ...state,
      row: 2,
      handled: true,
    });
    expect(alignTableKey(state, { key: "End" })).toEqual({
      ...state,
      row: 3,
      anchor: 3,
      handled: true,
    });
    expect(alignTableKey(state, { key: "m" })).toEqual({
      ...state,
      action: "merge",
      handled: true,
    });
    expect(alignTableKey(state, { key: "K" })).toEqual({
      ...state,
      action: "toggle-keep",
      handled: true,
    });
    expect(alignTableKey(state, { key: "e" })).toEqual({
      ...state,
      focusEditor: true,
      handled: true,
    });

    const started = alignTableKey(state, { key: " " });
    expect(started).toEqual({
      ...state,
      pinpoint: { row: 1, side: "source" },
      handled: true,
    });
    expect(
      alignTableKey(
        { ...started, row: 2, side: "source" },
        { key: " " },
      ),
    ).toEqual({
      ...started,
      row: 2,
      side: "source",
      handled: true,
    });
    expect(
      alignTableKey(
        { ...started, row: 2, side: "target" },
        { key: " " },
      ),
    ).toEqual({
      ...state,
      row: 2,
      side: "target",
      pinpoint: null,
      handled: true,
      action: "pinpoint",
      extra: {
        start_row: 1,
        end_row: 2,
        side: "source",
        end_side: "target",
      },
    });
    expect(
      alignTableKey(
        { ...started, side: "target" },
        { key: "Escape" },
      ),
    ).toEqual({
      ...state,
      side: "target",
      pinpoint: null,
      handled: true,
    });
  });

  it("models Java one-column drag/drop eligibility and request payloads", () => {
    const beads: AlignBead[] = [
      {
        source: "a b",
        target: "A",
        source_lines: ["a", "b"],
        target_lines: ["A"],
        score: 1,
        enabled: true,
        status: "accepted",
      },
      {
        source: "c",
        target: "C D",
        source_lines: ["c"],
        target_lines: ["C", "D"],
        score: 2,
        enabled: true,
        status: "needs-review",
      },
      {
        source: "e",
        target: "E",
        source_lines: ["e"],
        target_lines: ["E"],
        score: 3,
        enabled: true,
        status: "default",
      },
    ];

    expect(
      alignTableDrop(beads, {
        startRow: 1,
        endRow: 2,
        side: "source",
        targetRow: 4,
        targetSide: "source",
      }),
    ).toEqual({
      allowed: true,
      action: "move-to-row",
      extra: {
        start_row: 1,
        end_row: 2,
        side: "source",
        target_row: 4,
      },
    });
    expect(
      alignTableDrop(beads, {
        startRow: 1,
        endRow: 2,
        side: "source",
        targetRow: 4,
        targetSide: "target",
      }),
    ).toEqual({ allowed: false });
    expect(
      alignTableDrop(beads, {
        startRow: 0,
        endRow: 0,
        side: "source",
        targetRow: 4,
        targetSide: "source",
      }),
    ).toEqual({ allowed: false });
    expect(
      alignTableDrop(beads, {
        startRow: 3,
        endRow: 3,
        side: "source",
        targetRow: 4,
        targetSide: "source",
      }),
    ).toEqual({ allowed: false });
  });
});
