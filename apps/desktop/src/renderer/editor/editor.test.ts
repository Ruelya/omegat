import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

describe("segment editor source", () => {
  it("does not use contentEditable", () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const src = readFileSync(join(here, "SegmentEditor.tsx"), "utf8");
    expect(src).not.toMatch(/contentEditable/);
    expect(src).not.toMatch(/EditorController/);
    expect(src).toMatch(/role="textbox"/);
    expect(src).toMatch(/RendererPageProjection/);
    expect(src).toMatch(/rendererPage\.project/);
    expect(src).toMatch(/area\.handleBeforeInput\(native\.inputType, native\.data\)/);
    expect(src).toMatch(/renderedCaretFromPoint\(root, ev\.clientX, ev\.clientY\)/);
    expect(src).toMatch(/area\.beginMouseSelection\(hit\.offset, hit\.bias, ev\.shiftKey\)/);
    expect(src).toMatch(/interaction\.current\.updateMouseSelection\(hit\.offset, hit\.bias\)/);
    expect(src).toMatch(/onPointerUp=\{finishPointerSelection\}/);
    expect(src).toMatch(/scrollAdjustmentForAnchor/);
    expect(src).toMatch(/area\.pasteText\(text\)/);
  });
});
