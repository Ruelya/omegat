import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { ComesFromMTMarker } from "./mark/ComesFromMTMarker";
import { RemoveTagMarker } from "./mark/RemoveTagMarker";
import { ReplaceMarker } from "./mark/ReplaceMarker";
import { createFilterDocument, replace } from "./DocumentFilter3";
import { EditorController } from "./EditorController";
import { EditorDocumentLifecycle } from "./EditorDocumentLifecycle";
import { SegmentExportImport } from "./SegmentExportImport";
import { CharTableModel } from "./chartable/CharTableModel";
import { ARROW_COLLAPSED, CollapsibleBar } from "./CollapsibleBar";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../../fixtures/goldens/editor");

function load(name: string) {
  return JSON.parse(readFileSync(join(goldDir, name), "utf8"));
}

function compact(
  marks: { startOffset: number; endOffset: number; entryPart: string; toolTipText?: string }[] | null,
  expected?: unknown,
) {
  if (marks == null) return null;
  const wantTip = Array.isArray(expected) && expected.some((m) => m && typeof m === "object" && "toolTipText" in m);
  return marks.map((m) => ({
    startOffset: m.startOffset,
    endOffset: m.endOffset,
    entryPart: m.entryPart,
    ...(wantTip && m.toolTipText ? { toolTipText: m.toolTipText } : {}),
  }));
}

describe("remaining editor Java *Test goldens", () => {
  it("ComesFromAutoTMMarkerTest methods assert_eq", () => {
    const off = new ComesFromAutoTMMarker();
    expect(off.getMarksForEntry({ sourceText: null, translationText: null, isActive: true })).toBeNull();
    expect(off.getMarksForEntry({ sourceText: null, translationText: null, isActive: false })).toBeNull();
    const g = load("ComesFromAutoTMMarkerTest#testMarkersAutoTM.json");
    const marks = new ComesFromAutoTMMarker().getMarksForEntry({
      sourceText: g.source,
      translationText: g.translation,
      isActive: true,
      fromAuto: true,
    });
    expect(compact(marks, g.marks)).toEqual(g.marks);
  });

  it("ComesFromMTMarkerTest methods assert_eq", () => {
    const m = new ComesFromMTMarker();
    expect(m.getMarksForEntry({ sourceText: null, translationText: null, isActive: true })).toBeNull();
    expect(
      m.getMarksForEntry({ sourceText: "source", translationText: "target", isActive: false }),
    ).toBeNull();
    const g = load("ComesFromMTMarkerTest#testMarkersMT.json");
    m.setMark("ste", g.translation);
    expect(
      compact(
        m.getMarksForEntry({
          sourceText: g.source,
          translationText: g.translation,
          isActive: true,
        }),
        g.marks,
      ),
    ).toEqual(g.marks);
  });

  it("ReplaceMarkerTest#testReplaceMarker assert_eq", () => {
    const g = load("ReplaceMarkerTest#testReplaceMarker.json");
    const marker = new ReplaceMarker();
    marker.needle = g.needle;
    const marks = marker.getMarksForEntry({
      sourceText: g.source,
      translationText: g.source,
      isActive: true,
    });
    expect(compact(marks, g.marks)).toEqual(g.marks);
  });

  it("RemoveTagMarkerTest#testRemoveTagMarker assert_eq", () => {
    const g = load("RemoveTagMarkerTest#testRemoveTagMarker.json");
    const marks = new RemoveTagMarker().getMarksForEntry({
      sourceText: g.source,
      translationText: g.translation,
      isActive: true,
    });
    expect(compact(marks, g.marks)).toEqual(g.marks);
  });

  it("DocumentFilter3Test replace methods assert_eq", () => {
    const allow = load("DocumentFilter3Test#testReplace_AllowsValidReplacement.json");
    const doc = createFilterDocument("0123456789", allow.translation_start, allow.translation_end);
    const r = replace(doc, allow.offset, allow.length, allow.text);
    expect(r.applied).toBe(allow.applied);

    const oob = load("DocumentFilter3Test#testReplace_DoesNotAllowReplacement_OutOfBounds.json");
    const d2 = createFilterDocument("0123456789", oob.translation_start, oob.translation_end);
    expect(replace(d2, oob.offset, oob.length, oob.text).applied).toBe(oob.applied);

    const trusted = load("DocumentFilter3Test#testReplace_TriggeredInTrustedMode.json");
    const d3 = { ...createFilterDocument("x", 0, 1), trustedChangesInProgress: true, editMode: false };
    expect(replace(d3, trusted.offset, trusted.length, trusted.text).applied).toBe(trusted.applied);

    const reject = load("DocumentFilter3Test#testReplace_RejectsWhenNotInEditMode.json");
    const d4 = { ...createFilterDocument("x", 0, 1), editMode: false };
    expect(replace(d4, reject.offset, reject.length, reject.text).applied).toBe(reject.applied);

    const composed = load("DocumentFilter3Test#testReplace_SetsTextBeingComposed.json");
    const d5 = createFilterDocument("0123456789", composed.translation_start, composed.translation_end);
    const r5 = replace(d5, composed.offset, composed.length, composed.text, { composed: true });
    expect(r5.applied).toBe(composed.applied);
    expect(r5.doc.textBeingComposed).toBe(composed.text_being_composed);
  });

  it("SegmentExportImportTest methods assert_eq", () => {
    const exp = load("SegmentExportImportTest#testSegmentExportCurrentSegment.json");
    const sei = new SegmentExportImport();
    sei.exportCurrentSegment({ source: exp.source, translation: exp.translation });
    expect(SegmentExportImport.exists("source.txt")).toBe(true);
    expect(SegmentExportImport.exists("target.txt")).toBe(true);
    expect(SegmentExportImport.read("source.txt")).toBe(exp.source);
    expect(SegmentExportImport.read("target.txt")).toBe(exp.translation);

    const flush = load("SegmentExportImportTest#testFlushExportedSegments.json");
    SegmentExportImport.flushExportedSegments();
    expect(SegmentExportImport.read("source.txt")).toBe(flush.after_flush);
    expect(SegmentExportImport.read("target.txt")).toBe(flush.after_flush);

    const sel = load("SegmentExportImportTest#testExportCurrentSelection.json");
    SegmentExportImport.exportCurrentSelection(sel.selection);
    expect(SegmentExportImport.read("selection.txt")).toBe(sel.selection);
  });

  it("EditorControllerTest methods assert_eq", () => {
    const c = new EditorController();
    const documents = new EditorDocumentLifecycle();
    const defaults = load("EditorControllerTest#testEditorControllerDefaults.json");
    expect(c.displayedFileIndex).toBe(defaults.displayed_file_index);

    const empty = load("EditorControllerTest#testEditorControllerLoadEmptyProject.json");
    documents.clear();
    c.loadEmptyProject();
    expect({
      orientation_all_ltr: c.isOrientationAllLtr(),
      document: documents.document,
    }).toEqual({
      orientation_all_ltr: empty.orientation_all_ltr,
      document: empty.document,
    });

    const simple = load("EditorControllerTest#testEditorControllerLoadSimpleProject.json");
    const lifecycleDocument = documents.activate(1, "XXX", "");
    c.loadSimpleProject();
    const doc = c.getOmDocument();
    expect({
      current_file: c.getCurrentFile(),
      current_entry_number: c.getCurrentEntryNumber(),
      translation_start: lifecycleDocument.translationStart,
      translation_end: lifecycleDocument.translationEnd,
    }).toEqual({
      current_file: simple.current_file,
      current_entry_number: simple.current_entry_number,
      translation_start: simple.translation_start,
      translation_end: simple.translation_end,
    });

    const caret = load("EditorControllerTest#testEditorControllerLoadSimpleProjectWithCaretEvent.json");
    expect({
      translation_start: documents.document!.translationStart,
      translation_end: documents.document!.translationEnd,
    }).toEqual({
      translation_start: caret.translation_start,
      translation_end: caret.translation_end,
    });
    expect(doc).toEqual(lifecycleDocument);
  });

  it("CharTableModel default grid matches Java constants", () => {
    const m = new CharTableModel(null);
    expect(m.getColumnCount()).toBe(16);
    expect(m.getValueAt(0, 0)).toBe(" ");
    expect(m.getValueAt(0, 1)).toBe("!");
    m.setData("ABBA");
    m.allowOnlyUnique();
    expect(m.getData()).toBe("AB");
    m.appendChar("C", true);
    expect(m.getData()).toBe("ABC");
  });

  it("CollapsibleBar starts collapsed with Java arrows", () => {
    class Sample extends CollapsibleBar {
      protected buildSummary(): string {
        return "summary";
      }
    }
    const bar = new Sample();
    expect(bar.isExpanded()).toBe(false);
    expect(bar.getArrow()).toBe(ARROW_COLLAPSED);
    bar.refreshSummary();
    expect(bar.getSummaryText()).toBe("summary");
    bar.toggle();
    expect(bar.isExpanded()).toBe(true);
  });
});
