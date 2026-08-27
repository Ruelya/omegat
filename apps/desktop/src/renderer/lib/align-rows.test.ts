import { describe, expect, it } from "vitest";
import { alignmentRows, selectionBounds, type AlignBead } from "./align-rows";

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
});
