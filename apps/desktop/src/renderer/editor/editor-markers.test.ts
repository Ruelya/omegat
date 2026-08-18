import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { MarkerController } from "./MarkerController";
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { WhitespaceMarker } from "./mark/WhitespaceMarker";
import { allMarkers } from "./mark/markers";
import type { MarkerInput } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../../fixtures/goldens/editor");

function loadGold(name: string) {
  return JSON.parse(readFileSync(join(goldDir, name), "utf8")) as Record<string, unknown>;
}

function compact(marks: Mark[] | null) {
  if (marks == null) return null;
  return marks.map((m) => ({
    startOffset: m.startOffset,
    endOffset: m.endOffset,
    entryPart: m.entryPart,
    ...(m.toolTipText ? { toolTipText: m.toolTipText } : {}),
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
      expect(compact(runNbsp(g)), name).toEqual(g.marks);
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
    expect(compact(m1)).toEqual(g1.marks);
    const g2 = loadGold("BiDiMarkersTest#testMarkersBidi2.json");
    const m2 = b.getMarksForEntry({
      sourceText: String(g2.source),
      translationText: String(g2.translation),
      isActive: true,
    })!;
    expect(compact(m2)).toEqual(g2.marks);
  });

  it("ProtectedPartsMarkerTest#testMarkerProtectedParts assert_eq", () => {
    const g = loadGold("ProtectedPartsMarkerTest#testMarkerProtectedParts.json");
    const marks = new ProtectedPartsMarker().getMarksForEntry({
      sourceText: String(g.source),
      translationText: null,
      isActive: true,
      protectedParts: g.protected_parts as { text: string; tooltip: string }[],
    });
    expect(compact(marks)).toEqual(g.marks);
  });

  it("AltTranslationsMarkerTest#testAltTranslationsMarker assert_eq", () => {
    const g = loadGold("AltTranslationsMarkerTest#testAltTranslationsMarker") as {
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
});
