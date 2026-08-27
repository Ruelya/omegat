import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

describe("segment editor source", () => {
  it("does not use contentEditable", () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const src = readFileSync(join(here, "SegmentEditor.tsx"), "utf8");
    expect(src).not.toMatch(/contentEditable/);
    expect(src).toMatch(/role="textbox"/);
    expect(src).toMatch(/synchronizeRendererProject/);
    expect(src).toMatch(/area\.deleteBackward\(\)/);
    expect(src).toMatch(/area\.pasteText\(text\)/);
  });
});
