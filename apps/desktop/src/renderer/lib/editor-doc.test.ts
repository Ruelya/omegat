import { describe, expect, it } from "vitest";
import {
  decorateText,
  extraFromMarks,
  marksFromExtra,
  nextMissingTag,
  parseDocument,
  pushUndo,
  redoDraft,
  switchCase,
  undoDraft,
} from "./editor-doc";

describe("segment document", () => {
  it("protects OmegaT shortcut tags as tokens", () => {
    const tokens = parseDocument("Hello <f0>world</f0> <x1/>");
    expect(tokens.map((t) => t.kind)).toEqual(["text", "tag", "text", "tag", "text", "tag"]);
    expect(nextMissingTag("A <f0>b</f0>", "A ")).toBe("<f0>");
    expect(nextMissingTag("A <f0>b</f0>", "A <f0>b</f0>")).toBeNull();
  });

  it("undo/redo is a document stack, not the DOM", () => {
    let stacks = { undo: [] as string[], redo: [] as string[] };
    stacks = pushUndo(stacks, "a", "ab");
    let draft = "ab";
    ({ draft, stacks } = undoDraft(stacks, draft));
    expect(draft).toBe("a");
    ({ draft, stacks } = redoDraft(stacks, draft));
    expect(draft).toBe("ab");
  });

  it("view mark prefs change decoration and persist", () => {
    const extra = extraFromMarks({
      ...marksFromExtra({}),
      whitespace: true,
      nbsp: true,
      glossary: true,
    });
    expect(extra.mark_whitespace).toBe("true");
    const marks = marksFromExtra(extra);
    const spans = decorateText("a \u00a0term", marks, ["term"]);
    expect(spans.some((s) => s.cls.includes("mark-ws"))).toBe(true);
    expect(spans.some((s) => s.cls.includes("mark-nbsp"))).toBe(true);
    expect(spans.some((s) => s.cls.includes("mark-glossary"))).toBe(true);
  });

  it("cycles case like the Java Edit menu", () => {
    expect(switchCase("hello", "title")).toBe("Hello");
    expect(switchCase("HELLO", "cycle")).toBe("hello");
  });
});
