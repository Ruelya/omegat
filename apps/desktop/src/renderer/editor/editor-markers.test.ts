import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { createDocument3 } from "./Document3";
import { EditorController } from "./EditorController";
import { javaTooltipAt, MarkerController } from "./MarkerController";
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { SpellCheckerMarker } from "./mark/SpellCheckerMarker";
import { WhitespaceMarker } from "./mark/WhitespaceMarker";
import { allMarkers } from "./mark/markers";
import type { MarkerInput } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../../fixtures/goldens/editor");

function loadGold(name: string) {
  return JSON.parse(readFileSync(join(goldDir, name), "utf8")) as Record<string, unknown>;
}

function compact(marks: Mark[] | null, expected?: unknown) {
  if (marks == null) return null;
  const wantTip = Array.isArray(expected) && expected.some((m) => m && typeof m === "object" && "toolTipText" in m);
  return marks.map((m) => ({
    startOffset: m.startOffset,
    endOffset: m.endOffset,
    entryPart: m.entryPart,
    ...(wantTip && m.toolTipText ? { toolTipText: m.toolTipText } : {}),
  }));
}

function runNbsp(g: Record<string, unknown>): Mark[] | null {
  const m = new NBSPMarker();
  m.enabled = g.enabled !== false;
  return m.getMarksForEntry({
    sourceText: String(g.source ?? ""),
    translationText: (g.translation as string | null) ?? null,
    isActive: g.is_active !== false,
  });
}

describe("editor markers vs Java-exported goldens", () => {
  it("every editor golden has java_test + ExportGoldens provenance", () => {
    const files = readdirSync(goldDir).filter((f) => f.endsWith(".json"));
    expect(files.length).toBeGreaterThan(20);
    for (const f of files) {
      const g = loadGold(f);
      expect(g.exported_by, f).toBe("org.omegat.tools.ExportGoldens");
      expect(String(g.java_test), f).toMatch(/^org\.omegat\./);
    }
  });

  it("NBSPMarkerTest methods assert_eq intervals", () => {
    expect(runNbsp(loadGold("NBSPMarkerTest#testMarkerDisabled.json"))).toBeNull();
    expect(compact(runNbsp(loadGold("NBSPMarkerTest#testMarkerNotActive.json")))).toEqual([]);
    for (const name of [
      "NBSPMarkerTest#testMarkerNBSP.json",
      "NBSPMarkerTest#testMarkerNarrowNBSP.json",
      "NBSPMarkerTest#testMarkerFigureSpace.json",
      "NBSPMarkerTest#testMarkerBothNoBreakSpaces.json",
    ]) {
      const g = loadGold(name);
      expect(compact(runNbsp(g), g.marks), name).toEqual(g.marks);
    }
  });

  it("WhitespaceMarkerTest methods assert_eq intervals", () => {
    const disabled = new WhitespaceMarker();
    disabled.enabled = false;
    expect(
      disabled.getMarksForEntry({ sourceText: "source text", translationText: null, isActive: true }),
    ).toBeNull();
    const ws = new WhitespaceMarker();
    const notActive = ws.getMarksForEntry({
      sourceText: "source",
      translationText: null,
      isActive: false,
    });
    expect(notActive).toEqual([]);
    const sp = loadGold("WhitespaceMarkerTest#testMarkersSP.json");
    const marks = ws.getMarksForEntry({
      sourceText: String(sp.source),
      translationText: String(sp.translation),
      isActive: true,
      displaySource: true,
    })!;
    expect(marks).toHaveLength(8);
    expect(marks[0]!.startOffset).toBe(6);
    expect(marks[0]!.endOffset).toBe(7);
    expect(marks[3]!.startOffset).toBe(17);
    expect(marks[3]!.endOffset).toBe(18);
    expect(marks[3]!.toolTipText).toBe("Tab");
    expect(marks[3]!.entryPart).toBe("SOURCE");
    const sp2 = loadGold("WhitespaceMarkerTest#testMarkersSP2.json");
    const marks2 = ws.getMarksForEntry({
      sourceText: String(sp2.source),
      translationText: String(sp2.translation),
      isActive: false,
      displaySource: false,
    })!;
    expect(marks2).toHaveLength(4);
    expect(marks2[0]!.startOffset).toBe(6);
    expect(marks2[3]!.startOffset).toBe(17);
    expect(marks2[3]!.toolTipText).toBe("Tab");
    expect(marks2[3]!.entryPart).toBe("TRANSLATION");
  });

  it("BiDiMarkersTest methods assert_eq intervals", () => {
    const off = new BidiMarkers();
    off.enabled = false;
    expect(off.getMarksForEntry({ sourceText: "source text", translationText: null, isActive: true })).toBeNull();
    const b = new BidiMarkers();
    expect(b.getMarksForEntry({ sourceText: "source text", translationText: null, isActive: false })).toEqual([]);
    expect(b.getMarksForEntry({ sourceText: "edit", translationText: "edit", isActive: true })).toEqual([]);
    const g1 = loadGold("BiDiMarkersTest#testMarkersBidi.json");
    const m1 = b.getMarksForEntry({
      sourceText: String(g1.source),
      translationText: String(g1.translation),
      isActive: true,
    })!;
    expect(compact(m1, g1.marks)).toEqual(g1.marks);
    const g2 = loadGold("BiDiMarkersTest#testMarkersBidi2.json");
    const m2 = b.getMarksForEntry({
      sourceText: String(g2.source),
      translationText: String(g2.translation),
      isActive: true,
    })!;
    expect(compact(m2, g2.marks)).toEqual(g2.marks);
  });

  it("ProtectedPartsMarkerTest#testMarkerProtectedParts assert_eq", () => {
    const g = loadGold("ProtectedPartsMarkerTest#testMarkerProtectedParts.json");
    const marks = new ProtectedPartsMarker().getMarksForEntry({
      sourceText: String(g.source),
      translationText: null,
      isActive: true,
      protectedParts: g.protected_parts as { text: string; tooltip: string }[],
    });
    expect(compact(marks, g.marks)).toEqual(g.marks);
  });

  it("AltTranslationsMarkerTest#testAltTranslationsMarker assert_eq", () => {
    const g = loadGold("AltTranslationsMarkerTest#testAltTranslationsMarker.json") as {
      default: { isAlt: boolean };
      alternative: { source: string; translation: string };
    };
    const marker = new AltTranslationsMarker();
    const def: MarkerInput = {
      sourceText: "Edit",
      translationText: "default",
      isActive: true,
      isAlt: false,
    };
    expect(marker.getMarksForEntry(def)).toBeNull();
    const alt = marker.getMarksForEntry({
      sourceText: g.alternative.source,
      translationText: g.alternative.translation,
      isActive: true,
      isAlt: true,
    });
    expect(alt).toHaveLength(1);
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

  it("registers, invalidates, recomputes, and unloads a plugin marker", () => {
    const ctrl = new MarkerController();
    let enabled = true;
    let calls = 0;
    const plugin = {
      getMarksForEntry(input: MarkerInput): Mark[] {
        calls += 1;
        return enabled && input.translationText
          ? [{
              startOffset: 0,
              endOffset: 1,
              painter: "plugin",
              toolTipText: "plugin marker",
              entryPart: "TRANSLATION",
            }]
          : [];
      },
    };
    const input: MarkerInput = {
      sourceText: "source",
      translationText: "x",
      isActive: true,
    };

    ctrl.registerPluginMarker("example.PluginMarker", plugin);
    expect(ctrl.getMarkerNames().at(-1)).toBe("example.PluginMarker");
    const first = ctrl.applyToDocument(
      "entry",
      createDocument3("source", "x"),
      input,
    );
    expect({
      calls,
      marks: first.snapshot.marks.filter((mark) => mark.painter === "plugin"),
      spans: first.document.spans.filter((span) => span.style === "marker:plugin"),
    }).toEqual({
      calls: 1,
      marks: [{
        startOffset: 0,
        endOffset: 1,
        painter: "plugin",
        toolTipText: "plugin marker",
        entryPart: "TRANSLATION",
      }],
      spans: [{
        start: first.document.translationStart,
        end: first.document.translationStart + 1,
        style: "marker:plugin",
      }],
    });

    const cached = ctrl.processEntry("entry", input);
    expect({ calls, generation: cached.generation }).toEqual({
      calls: 1,
      generation: first.snapshot.generation,
    });

    enabled = false;
    ctrl.remarkOneMarker("example.PluginMarker");
    const recomputed = ctrl.applyToDocument("entry", first.document, input);
    expect({
      calls,
      generationAdvanced: recomputed.snapshot.generation > cached.generation,
      pluginMarks: recomputed.snapshot.marks.filter((mark) => mark.painter === "plugin"),
      pluginSpans: recomputed.document.spans.filter((span) => span.style === "marker:plugin"),
    }).toEqual({
      calls: 2,
      generationAdvanced: true,
      pluginMarks: [],
      pluginSpans: [],
    });

    expect(ctrl.unregisterPluginMarker("example.PluginMarker")).toBe(true);
    expect(ctrl.unregisterPluginMarker("example.PluginMarker")).toBe(false);
    expect(ctrl.getMarkerNames().includes("example.PluginMarker")).toBe(false);
    expect(ctrl.unregisterPluginMarker("NBSPMarker")).toBe(false);
    expect(() => ctrl.registerPluginMarker("NBSPMarker", plugin)).toThrow(
      "marker already registered: NBSPMarker",
    );
  });

  it("returns Java-shaped overlapping marker tooltips at a UTF-16 hit", () => {
    const marks: Mark[] = [
      {
        startOffset: 1,
        endOffset: 4,
        painter: "first",
        toolTipText: "plain",
        entryPart: "TRANSLATION",
      },
      {
        startOffset: 3,
        endOffset: 6,
        painter: "second",
        toolTipText: "<suggestion>replacement</suggestion>",
        entryPart: "TRANSLATION",
      },
      {
        startOffset: 3,
        endOffset: 6,
        painter: "source-only",
        toolTipText: "source",
        entryPart: "SOURCE",
      },
    ];
    expect(javaTooltipAt(marks, "TRANSLATION", 3)).toBe(
      "<html>plain<br><b>replacement</b></html>",
    );
    expect(javaTooltipAt(marks, "TRANSLATION", 7)).toBeNull();
  });

  it("discards an asynchronous marker callback from an older translation generation", async () => {
    const ctrl = new MarkerController();
    const pending: ((marks: Mark[]) => void)[] = [];
    ctrl.registerPluginMarker("example.AsyncMarker", {
      getMarksForEntryAsync: () =>
        new Promise<Mark[]>((resolve) => {
          pending.push(resolve);
        }),
    });
    const input = (translationText: string): MarkerInput => ({
      sourceText: "source",
      translationText,
      isActive: true,
    });

    const stale = ctrl.processEntryAsync("entry", input("old"));
    const current = ctrl.processEntryAsync("entry", input("new text"));
    expect(pending).toHaveLength(2);
    pending[1]!([{
      startOffset: 4,
      endOffset: 8,
      painter: "async-current",
      entryPart: "TRANSLATION",
    }]);
    await current;
    pending[0]!([{
      startOffset: 0,
      endOffset: 3,
      painter: "async-stale",
      entryPart: "TRANSLATION",
    }]);
    await stale;

    expect(ctrl.getCached("entry")!.marks.filter((mark) => mark.painter.startsWith("async"))).toEqual([{
      startOffset: 4,
      endOffset: 8,
      painter: "async-current",
      entryPart: "TRANSLATION",
    }]);
  });

  it("recomputes only the remarked async marker and expires its prior callback", async () => {
    const ctrl = new MarkerController();
    const firstPending: ((marks: Mark[]) => void)[] = [];
    const secondPending: ((marks: Mark[]) => void)[] = [];
    ctrl.registerPluginMarker("example.FirstAsyncMarker", {
      getMarksForEntryAsync: () =>
        new Promise<Mark[]>((resolve) => {
          firstPending.push(resolve);
        }),
    });
    ctrl.registerPluginMarker("example.SecondAsyncMarker", {
      getMarksForEntryAsync: () =>
        new Promise<Mark[]>((resolve) => {
          secondPending.push(resolve);
        }),
    });
    const input: MarkerInput = {
      sourceText: "source",
      translationText: "current",
      isActive: true,
    };

    const initial = ctrl.processEntryAsync("entry", input);
    firstPending[0]!([]);
    secondPending[0]!([]);
    await initial;

    ctrl.remarkOneMarker("example.FirstAsyncMarker");
    const stale = ctrl.processEntryAsync("entry", input);
    ctrl.remarkOneMarker("example.FirstAsyncMarker");
    const current = ctrl.processEntryAsync("entry", input);
    expect({
      firstCalls: firstPending.length,
      secondCalls: secondPending.length,
    }).toEqual({
      firstCalls: 3,
      secondCalls: 1,
    });

    firstPending[2]!([{
      startOffset: 4,
      endOffset: 7,
      painter: "remark-current",
      entryPart: "TRANSLATION",
    }]);
    await current;
    firstPending[1]!([{
      startOffset: 0,
      endOffset: 3,
      painter: "remark-stale",
      entryPart: "TRANSLATION",
    }]);
    await stale;

    expect(
      ctrl.getCached("entry")!.marks.filter((mark) => mark.painter.startsWith("remark-")),
    ).toEqual([{
      startOffset: 4,
      endOffset: 7,
      painter: "remark-current",
      entryPart: "TRANSLATION",
    }]);
  });

  it("expires asynchronous Marker work when an inactive entry leaves the loaded page", async () => {
    const controller = new EditorController();
    const pending: {
      input: MarkerInput;
      resolve: (marks: Mark[]) => void;
    }[] = [];
    controller.registerPluginMarker("example.InactiveAsyncMarker", {
      getMarksForEntryAsync: (input) =>
        new Promise<Mark[]>((resolve) => {
          pending.push({ input, resolve });
        }),
    });
    controller.setPageRadius(1);
    controller.loadProject([
      { file: "a.txt", id: "first", source: "first", translation: "un" },
      { file: "a.txt", id: "second", source: "second", translation: "deux" },
      { file: "b.txt", id: "third", source: "third", translation: "trois" },
    ], 2);
    const initialPage = controller.getLoadedPage();
    const stalePage = controller.refreshLoadedPageMarkersAsync();
    expect(pending.map(({ input }) => ({
      source: input.sourceText,
      active: input.isActive,
    }))).toEqual([
      { source: "first", active: false },
      { source: "second", active: true },
      { source: "third", active: false },
    ]);

    controller.setPageRadius(0);
    pending[0]!.resolve([{
      startOffset: 0,
      endOffset: 2,
      painter: "inactive-stale-first",
      entryPart: "TRANSLATION",
    }]);
    pending[1]!.resolve([{
      startOffset: 0,
      endOffset: 4,
      painter: "active-current",
      entryPart: "TRANSLATION",
    }]);
    pending[2]!.resolve([{
      startOffset: 0,
      endOffset: 5,
      painter: "inactive-stale-third",
      entryPart: "TRANSLATION",
    }]);

    expect(await stalePage).toBe(false);
    expect({
      first: controller.markers.getCached(initialPage[0]!.key),
      third: controller.markers.getCached(initialPage[2]!.key),
      activeBeforeApply: controller.getOmDocument()?.spans
        .filter(({ style }) => style.startsWith("marker:"))
        .map(({ style }) => style),
    }).toEqual({
      first: null,
      third: null,
      activeBeforeApply: [],
    });

    expect(await controller.refreshLoadedPageMarkersAsync()).toBe(true);
    expect({
      page: controller.getLoadedPage().map(({ entryNumber, marks }) => ({
        entryNumber,
        painters: marks.map(({ painter }) => painter),
      })),
      document: controller.getOmDocument()?.spans
        .filter(({ style }) => style.startsWith("marker:"))
        .map(({ style }) => style),
    }).toEqual({
      page: [{
        entryNumber: 2,
        painters: ["active-current"],
      }],
      document: ["marker:active-current"],
    });
  });

  it("bridges sidecar spell tokens into Java-style translation marks", async () => {
    const calls: string[] = [];
    const marker = new SpellCheckerMarker(async (text) => {
      calls.push(text);
      return [{ word: "wrng", offset: 3, length: 4 }];
    });
    expect(await marker.getMarksForEntryAsync({
      sourceText: "source",
      translationText: "😀 wrng",
      isActive: true,
    })).toEqual([{
      startOffset: 3,
      endOffset: 7,
      painter: "spell",
      entryPart: "TRANSLATION",
    }]);
    expect(calls).toEqual(["😀 wrng"]);
  });
});
