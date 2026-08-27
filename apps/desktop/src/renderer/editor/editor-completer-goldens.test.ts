import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GlossaryAutoCompleterView } from "./autocompleter/GlossaryAutoCompleterView";
import { CharTableAutoCompleterView } from "./chartable/CharTableAutoCompleterView";
import { ZERO_WIDTH_SPACE } from "./chartable/CharTableModel";
import { CollapsibleBar } from "./CollapsibleBar";
import { EditorController } from "./EditorController";
import { FilterBypass } from "./FilterBypass";
import { createFilterDocument, replace } from "./DocumentFilter3";
import { isFromMTMemory } from "../core/data/DataUtils";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { EditorColor } from "./mark/EditorColor";
import { buildActiveDocument, createSegmentMarkText } from "./SegmentBuilder";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../../fixtures/goldens/editor");

function load(name: string) {
  return JSON.parse(readFileSync(join(goldDir, name), "utf8"));
}

describe("completer / color / insertString Java goldens", () => {
  it("GlossaryAutoCompleterViewTest#testSuggestions assert_eq", () => {
    const g = load("GlossaryAutoCompleterViewTest#testSuggestions.json");
    const view = new GlossaryAutoCompleterView();
    expect(view.computeListData("blah", false).map((i) => i.payload)).toEqual([]);
    view.entries = g.terms.map((t: string) => ({ source: "", locTerms: [t] }));
    for (const c of g.cases) {
      expect(view.computeListData(c.chunk, c.contextual_only).map((i) => i.payload), JSON.stringify(c)).toEqual(
        c.payloads,
      );
    }
  });

  it("CharTableModelTest methods assert_eq", () => {
    const zwsp = load("CharTableModelTest#defaultTableIncludesZeroWidthSpace.json");
    const view = new CharTableAutoCompleterView(null);
    expect(view.model.getColumnCount()).toBe(zwsp.columns);
    let found = { x: -1, y: -1 };
    for (let y = 0; y < view.model.getRowCount(); y++) {
      for (let x = 0; x < view.model.getColumnCount(); x++) {
        if (view.model.getValueAt(y, x) === zwsp.glyph) found = { x, y };
      }
    }
    expect(found.x).toBeGreaterThanOrEqual(0);
    expect(view.model.getValueAt(found.y, found.x)).toBe(ZERO_WIDTH_SPACE);
    const sel = load("CharTableModelTest#autoCompleterSelectionUsesZeroWidthSpacePayload.json");
    view.setSelection(found);
    expect(view.getSelectedValue()?.payload).toBe(sel.payload);
  });

  it("CollapsibleBarTest methods assert_eq", () => {
    class TestBar extends CollapsibleBar {
      summaryText = "empty";
      constructor() {
        super();
        this.refreshSummary();
      }
      protected buildSummary(): string {
        return this.summaryText;
      }
    }
    const start = load("CollapsibleBarTest#startsCollapsedByDefault.json");
    const bar = new TestBar();
    expect(bar.isExpanded()).toBe(start.expanded);
    const tog = load("CollapsibleBarTest#toggleExpandsAndCollapses.json");
    bar.toggle();
    expect(bar.isExpanded()).toBe(tog.after_toggle);
    bar.toggle();
    expect(bar.isExpanded()).toBe(tog.after_second_toggle);
    const set = load("CollapsibleBarTest#setExpandedControlsState.json");
    bar.setExpanded(true);
    expect(bar.isExpanded()).toBe(set.set_true);
    bar.setExpanded(false);
    expect(bar.isExpanded()).toBe(set.set_false);
    const sum = load("CollapsibleBarTest#summaryReflectsModelAfterRefresh.json");
    expect(bar.getSummaryText()).toBe(sum.initial);
    bar.summaryText = sum.after;
    bar.refreshSummary();
    expect(bar.getSummaryText()).toBe(sum.after);
    const ctor = load("CollapsibleBarTest#constructorDoesNotCallBuildSummaryBeforeSubclassInit.json");
    expect(new TestBar().getSummaryText()).toBe(ctor.summary);
  });

  it("MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange assert_eq", () => {
    const g = load("MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange.json");
    const marker = new ComesFromAutoTMMarker();
    const color = EditorColor.COLOR_MARK_COMES_FROM_TM_XAUTO;
    const before = marker.getMarksForEntry({
      sourceText: g.source,
      translationText: g.translation,
      isActive: true,
      fromAuto: true,
      linked: g.linked,
    });
    expect(before?.[0]?.painterColor).toBe(g.before_color);
    expect(color.getColor()).toBe(g.before_color);
    color.setColor(g.after_color);
    const after = marker.getMarksForEntry({
      sourceText: g.source,
      translationText: g.translation,
      isActive: true,
      fromAuto: true,
      linked: g.linked,
    });
    expect(after?.[0]?.painterColor).toBe(g.after_color);
    color.setColor(null);
  });

  it("ComesFromMTMarkerTest#testNearString assert_eq", () => {
    const g = load("ComesFromMTMarkerTest#testNearString.json");
    expect(isFromMTMemory({ comesFrom: g.comes_from, projs: [g.proj] }, g.tm_root)).toBe(g.from_mt);
  });

  it("EditorController insertString offset assert_eq", () => {
    const simple = load("EditorControllerTest#testEditorControllerLoadSimpleProject.json");
    const doc = buildActiveDocument(1, "XXX", "");
    expect(doc.translationStart).toBe(simple.translation_start);
    expect(doc.translationEnd).toBe(simple.translation_end);
    expect(doc.fullText.startsWith("XXX\n")).toBe(true);
    expect(doc.fullText.includes(createSegmentMarkText(1, true))).toBe(true);
    const c = new EditorController();
    c.loadSimpleProject();
    expect(c.getOmDocument()?.translationStart).toBe(simple.translation_start);
    const leak = load("EditorProjectReloadLeakTest#closedProjectsMustBecomeUnreachableWithEditorAttached.json");
    for (let i = 0; i < leak.cycles; i++) {
      c.loadSimpleProject();
      c.closeProject();
    }
    expect(c.getOmDocument()).toBe(leak.document_after_close);
    expect(c.entries.length).toBe(leak.entry_count_after_close);
  });

  it("DocumentFilter3 uses FilterBypass.replace", () => {
    const doc = createFilterDocument("0123456789", 3, 7);
    const fb = new FilterBypass(doc);
    const r = replace(doc, 3, 2, "AB", null, fb);
    expect(r.applied).toBe(true);
    expect(r.bypass.doc.text.slice(3, 5)).toBe("AB");
    expect(r.doc).toBe(r.bypass.getDocument());
  });
});
