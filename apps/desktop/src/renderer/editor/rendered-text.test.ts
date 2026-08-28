import { describe, expect, it } from "vitest";
import { DEFAULT_MARKS } from "../lib/editor-doc";
import {
  javaTooltipOverRange,
  MarkerController,
} from "./MarkerController";
import { buildRenderedTextFragments } from "./RenderedText";
import { modelOffsetForRenderedPosition } from "./RenderedTextHitTest";
import type { Mark } from "./mark/Mark";

describe("decorated editor text product path", () => {
  it("keeps exact UTF-16 intervals across stacked markers, spelling, BiDi, and tags", () => {
    const marks: Mark[] = [
      {
        startOffset: 0,
        endOffset: 13,
        painter: "broad",
        toolTipText: "outer",
        entryPart: "TRANSLATION",
      },
      {
        startOffset: 3,
        endOffset: 8,
        painter: "protected",
        toolTipText: "protected tag",
        entryPart: "TRANSLATION",
      },
      {
        startOffset: 4,
        endOffset: 7,
        painter: "native-plugin",
        toolTipText: "nested marker",
        entryPart: "TRANSLATION",
      },
      {
        startOffset: 9,
        endOffset: 13,
        painter: "spell",
        toolTipText: "misspelled",
        entryPart: "TRANSLATION",
      },
    ];
    const fragments = buildRenderedTextFragments(
      "A\u200e <x0/> wrng",
      0,
      { ...DEFAULT_MARKS, bidi: true, whitespace: true, glossary: true },
      ["wrng"],
      marks,
    );

    expect(fragments.map((fragment) => ({
      text: fragment.text,
      offset: fragment.offset,
      sourceLength: fragment.sourceLength,
      atomic: fragment.atomic,
      classes: fragment.classes,
      tooltips: fragment.tooltipTexts,
    }))).toEqual([
      {
        text: "A",
        offset: 0,
        sourceLength: 1,
        atomic: false,
        classes: ["product-marker-broad"],
        tooltips: ["outer"],
      },
      {
        text: "LRM",
        offset: 1,
        sourceLength: 1,
        atomic: true,
        classes: ["mark-bidi", "product-marker-broad"],
        tooltips: ["outer"],
      },
      {
        text: "·",
        offset: 2,
        sourceLength: 1,
        atomic: false,
        classes: ["mark-ws", "product-marker-broad"],
        tooltips: ["outer"],
      },
      {
        text: "<x0/>",
        offset: 3,
        sourceLength: 5,
        atomic: true,
        classes: [
          "tag",
          "tag-protected",
          "product-marker-broad",
          "product-marker-protected",
          "product-marker-native-plugin",
        ],
        tooltips: ["outer", "protected tag", "nested marker"],
      },
      {
        text: "·",
        offset: 8,
        sourceLength: 1,
        atomic: false,
        classes: ["mark-ws", "product-marker-broad"],
        tooltips: ["outer"],
      },
      {
        text: "wrng",
        offset: 9,
        sourceLength: 4,
        atomic: false,
        classes: ["mark-glossary", "product-marker-broad", "mark-spell"],
        tooltips: ["outer", "misspelled"],
      },
    ]);
    expect(javaTooltipOverRange(marks, "TRANSLATION", 3, 8)).toBe(
      "<html>outer<br>protected tag<br>nested marker</html>",
    );
  });

  it("maps visible expansion and protected fragments to model-side boundaries", () => {
    expect(modelOffsetForRenderedPosition({
      offset: 1,
      sourceLength: 1,
      renderedLength: 3,
      atomic: true,
    }, 1)).toEqual({
      offset: 1,
      bias: "before",
      fragmentStart: 1,
      fragmentEnd: 2,
    });
    expect(modelOffsetForRenderedPosition({
      offset: 1,
      sourceLength: 1,
      renderedLength: 3,
      atomic: true,
    }, 2)).toEqual({
      offset: 2,
      bias: "after",
      fragmentStart: 1,
      fragmentEnd: 2,
    });
    expect(modelOffsetForRenderedPosition({
      offset: 3,
      sourceLength: 5,
      renderedLength: 5,
      atomic: true,
    }, 2)).toEqual({
      offset: 3,
      bias: "before",
      fragmentStart: 3,
      fragmentEnd: 8,
    });
    expect(modelOffsetForRenderedPosition({
      offset: 9,
      sourceLength: 4,
      renderedLength: 4,
    }, 2)).toEqual({
      offset: 11,
      bias: "after",
      fragmentStart: 9,
      fragmentEnd: 13,
    });
  });

  it("keeps source and inactive-entry tooltips isolated by key and entry part", () => {
    const controller = new MarkerController();
    controller.registerPluginMarker("example.SourceAndContextMarker", {
      getMarksForEntry: ({ isActive }) => [
        {
          startOffset: 0,
          endOffset: 5,
          painter: "source-tip",
          toolTipText: isActive ? "active source" : "inactive source",
          entryPart: "SOURCE",
        },
        {
          startOffset: 0,
          endOffset: 4,
          painter: "target-tip",
          toolTipText: isActive ? "active target" : "inactive target",
          entryPart: "TRANSLATION",
        },
      ],
    });
    controller.processEntry("active-key", {
      sourceText: "alpha",
      translationText: "beta",
      isActive: true,
    });
    controller.processEntry("context-key", {
      sourceText: "gamma",
      translationText: "delta",
      isActive: false,
    });

    expect({
      activeSource: controller.getToolTipsOverRange("active-key", "SOURCE", 0, 5),
      activeTarget: controller.getToolTipsOverRange("active-key", "TRANSLATION", 0, 4),
      contextSource: controller.getToolTipsOverRange("context-key", "SOURCE", 0, 5),
      contextTarget: controller.getToolTipsOverRange("context-key", "TRANSLATION", 0, 4),
      wrongPart: controller.getToolTipsOverRange("context-key", "SOURCE", 5, 9),
      wrongKey: controller.getToolTipsOverRange("missing-key", "SOURCE", 0, 5),
    }).toEqual({
      activeSource: "<html>active source</html>",
      activeTarget: "<html>active target</html>",
      contextSource: "<html>inactive source</html>",
      contextTarget: "<html>inactive target</html>",
      wrongPart: null,
      wrongKey: null,
    });
  });
});
