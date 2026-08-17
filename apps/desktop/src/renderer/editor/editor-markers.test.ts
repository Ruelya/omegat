import { describe, expect, it } from "vitest";
import { MarkerController } from "./MarkerController";
import {
  allMarkers,
  bidiMarkers,
  nbspMarker,
  protectedPartsMarker,
  whitespaceMarker,
} from "./mark/markers";

describe("editor markers (Java mark/* intervals)", () => {
  it("marks NBSP, whitespace, bidi, and protected tags with assert_eq intervals", () => {
    const text = "A\u00a0B <x0/> C\u200e";
    const ws = whitespaceMarker(text);
    expect(ws.some((m) => m.painter === "·")).toBe(true);
    expect(nbspMarker(text).map((m) => [m.startOffset, m.endOffset])).toEqual([[1, 2]]);
    expect(bidiMarkers(text).map((m) => m.painter)).toEqual(["bidi"]);
    expect(protectedPartsMarker(text).map((m) => m.toolTipText)).toEqual(["<x0/>"]);
  });

  it("composes marker kinds for a sample segment", () => {
    const marks = allMarkers({
      text: "Hello\u00a0<x1/>",
      source: "Hello <x1/>",
      isAlt: true,
      fromAuto: true,
    });
    expect(marks.filter((m) => m.kind === "nbsp").map((m) => [m.start, m.end])).toEqual([[5, 6]]);
    expect(marks.some((m) => m.kind === "protected")).toBe(true);
    expect(marks.some((m) => m.kind === "alt")).toBe(true);
    expect(marks.some((m) => m.kind === "auto-tm")).toBe(true);
  });

  it("MarkerController runs the Java marker set", () => {
    const ctrl = new MarkerController();
    const marks = ctrl.process({
      sourceText: "Hi <x0/>",
      translationText: "Bonjour\u00a0<x0/>",
      isActive: true,
      isAlt: true,
    });
    expect(marks.some((m) => m.painter === "nbsp" || m.toolTipText === "NBSP")).toBe(true);
    expect(marks.some((m) => m.painter === "protected" || m.toolTipText === "<x0/>")).toBe(true);
  });
});
