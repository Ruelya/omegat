import { describe, expect, it } from "vitest";
import {
  alignmentRows,
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
});
