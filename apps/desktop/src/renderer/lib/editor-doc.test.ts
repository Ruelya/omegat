import { describe, expect, it } from "vitest";
import {
  decorateText,
  deleteBackwardAtomic,
  deleteRangeAtomic,
  insertAtomic,
  marksFromPrefs,
  nextMissingTag,
  parseDocument,
  prefsFromMarks,
  pushUndo,
  redoDraft,
  snapCaret,
  switchCase,
  tagsIntact,
  undoDraft,
} from "./editor-doc";
import { defaultMarks } from "./preferences";

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

  it("view mark prefs change decoration and persist as typed marks", () => {
    const marks = marksFromPrefs({ ...defaultMarks(), whitespace: true, nbsp: true, glossary: true });
    expect(prefsFromMarks(marks).whitespace).toBe(true);
    const spans = decorateText("a \u00a0term", marks, ["term"]);
    expect(spans.some((s) => s.cls.includes("mark-ws"))).toBe(true);
    expect(spans.some((s) => s.cls.includes("mark-nbsp"))).toBe(true);
    expect(spans.some((s) => s.cls.includes("mark-glossary"))).toBe(true);
  });

  it("cycles case like the Java Edit menu", () => {
    expect(switchCase("hello", "title")).toBe("Hello");
    expect(switchCase("HELLO", "cycle")).toBe("hello");
  });

  it("treats tags as atomic: backspace and mid-tag insert cannot split them", () => {
    const src = "Hello <f0>world</f0>";
    const tagStart = src.indexOf("<f0>");
    const inside = tagStart + 2;
    const afterDelete = deleteBackwardAtomic(src, tagStart + "<f0>".length);
    expect(afterDelete.text).toBe("Hello world</f0>");
    expect(afterDelete.text.includes("<f0>")).toBe(false);
    expect(tagsIntact(afterDelete.text)).toBe(true);

    const inserted = insertAtomic(src, inside, "X");
    expect(inserted.text.includes("<f0>")).toBe(true);
    expect(inserted.text.includes("<fX0>") || inserted.text.includes("<f0X>")).toBe(false);
    expect(snapCaret(src, inside)).toBe(tagStart + "<f0>".length);

    const ranged = deleteRangeAtomic(src, inside, inside + 1);
    expect(ranged.text).toBe("Hello world</f0>");
    expect(ranged.text.includes("<f0>")).toBe(false);
    expect(tagsIntact(ranged.text)).toBe(true);
  });
});
