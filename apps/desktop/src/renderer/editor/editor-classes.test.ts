import { describe, expect, it } from "vitest";
import { AutoCompleter } from "./autocompleter/AutoCompleter";
import { GlossaryAutoCompleterView } from "./autocompleter/GlossaryAutoCompleterView";
import { AutotextAutoCompleterView } from "./autotext/AutotextAutoCompleterView";
import {
  commitAndDeactivate,
  createDocument3,
  Document3,
  extractTranslation,
  insertText,
  replaceEditText,
} from "./Document3";
import { allowInsert } from "./DocumentFilter3";
import { EditorController } from "./EditorController";
import { EditorTextArea3 } from "./EditorTextArea3";
import { HistoryPredictor } from "./history/HistoryPredictor";
import { HistoryCompleter } from "./history/HistoryCompleter";
import { IEditor } from "./IEditor";
import { makeFilter } from "./IEditorFilter";
import { TagAutoCompleterView } from "./TagAutoCompleterView";
import { TranslationUndoManager } from "./TranslationUndoManager";

describe("Document3 / IEditor / completer classes", () => {
  it("Document3 tracks active translation range and dirty flag", () => {
    let doc = createDocument3("src", "tgt");
    expect(doc.activeEnd).toBe(3);
    expect(doc.dirty).toBe(false);
    doc = insertText(doc, "X", 3);
    expect(doc.translation).toBe("tgtX");
    expect(doc.dirty).toBe(true);
    doc = replaceEditText(doc, "new");
    expect(doc.translation).toBe("new");
    doc = commitAndDeactivate(doc);
    expect(doc.dirty).toBe(false);
  });

  it("DocumentFilter3 refuses inserts inside tags", () => {
    expect(allowInsert("a<x0/>b", 3)).toBe(false);
    expect(allowInsert("a<x0/>b", 1)).toBe(true);
  });

  it("IEditor method table is complete (no empty break)", () => {
    const names = Object.keys(IEditor).sort();
    expect(names).toContain("commitAndDeactivate");
    expect(names).toContain("replaceEditTextAndMark");
    expect(names).toContain("registerUntranslated");
    expect(names).toContain("gotoEntry");
    expect(names).toContain("insertTag");
    expect(names).toContain("setFilter");
    expect(names.length).toBeGreaterThanOrEqual(50);
  });

  it("EditorController writes through IEditor", () => {
    const c = new EditorController();
    c.replaceEditText("hello");
    expect(c.getCurrentTranslation()).toBe("hello");
  });

  it("five autocompleter views return insertable items", () => {
    const g = new GlossaryAutoCompleterView();
    g.terms = [{ source: "cat", target: "chat" }];
    expect(g.computeListData("ca")[0]?.payload).toBe("chat");

    const a = new AutotextAutoCompleterView("om=OmegaT=tool");
    expect(a.computeListData("om")[0]?.payload).toBe("OmegaT");

    const h = new HistoryCompleter();
    h.translations = ["Bonjour le monde"];
    expect(h.computeListData("mon")[0]?.payload).toBe("monde");

    const p = new HistoryPredictor();
    p.train(["Hello world today", "Hello world tonight"]);
    expect(p.computeListData("Hello ").some((i) => i.payload === "world")).toBe(true);

    const t = new TagAutoCompleterView(["<x0/>", "<x1/>"]);
    expect(t.computeListData("x0")[0]?.payload).toBe("<x0/>");

    const ac = new AutoCompleter();
    ac.setViews([g, a, h, p, t]);
    expect(ac.updatePopup("ca").length).toBeGreaterThan(0);
    expect(ac.confirm()).toBe("chat");
  });

  it("TranslationUndoManager restores prior text", () => {
    const u = new TranslationUndoManager();
    u.remember("one");
    expect(u.undo("two")).toBe("one");
    expect(u.redo("one")).toBe("two");
  });

  it("Document3 mutable facade enforces bounds and trusted chrome edits", () => {
    const model = new Document3(createDocument3("source", "target"));
    expect(model.extractTranslation()).toBe("target");
    expect(model.insertString(3, "X")).toBe(true);
    expect(model.extractTranslation()).toBe("tarXget");
    expect(() => model.insertString(99, "bad")).toThrow("BadLocationException");

    model.setTrustedChangesInProgress(true);
    expect(model.insertString(0, "chrome:")).toBe(true);
    expect(model.getTranslationStart()).toBe(7);
    expect(model.extractTranslation()).toBe("tarXget");
    model.setAlignment(model.getTranslationStart(), model.getTranslationEnd(), true);
    expect(model.state.spans).toEqual([
      {
        start: model.getTranslationStart(),
        end: model.getTranslationEnd(),
        style: "align-right",
      },
    ]);
    model.stopEditMode();
    expect(model.isEditMode()).toBe(false);
    expect(model.extractTranslation()).toBeNull();
    expect(extractTranslation(model.state)).toBeNull();
  });

  it("EditorTextArea3 keeps tags atomic, selection, overtype and popup priority", () => {
    const area = new EditorTextArea3("source", "a<x0/>bc");
    area.selectAll();
    expect(area.getSelectedText()).toBe("a<x0/>bc");
    area.setSelection(2, 3);
    expect(area.deleteSelectionAtomic()).toBe(true);
    expect(area.getText()).toBe("abc");
    area.setCaretPosition(1);
    expect(area.toggleOvertype()).toBe(true);
    expect(area.insertText("Z")).toBe(true);
    expect(area.getText()).toBe("aZc");
    expect(area.deleteBackward()).toBe(true);
    expect(area.getText()).toBe("ac");
    expect(area.deleteForward()).toBe(true);
    expect(area.getText()).toBe("a");

    const calls: number[] = [];
    area.registerPopupMenuConstructor({
      priority: 20,
      build: () => {
        calls.push(20);
        return ["late"];
      },
    });
    area.registerPopupMenuConstructor({
      priority: 10,
      build: () => {
        calls.push(10);
        return ["early"];
      },
    });
    expect(area.makePopupMenu()).toEqual(["early", "late"]);
    expect(calls).toEqual([10, 20]);
  });

  it("EditorTextArea3 treats IME updates as one replaceable composition", () => {
    const area = new EditorTextArea3("source", "a<x0/>b");
    area.setCaretPosition(area.getOmDocument().translationEnd);
    expect(area.beginComposition()).toBe(true);
    expect(area.getOmDocument().textBeingComposed).toBe(true);
    expect(area.updateComposition("あ")).toBe(true);
    expect(area.updateComposition("あい")).toBe(true);
    expect(area.getText()).toBe("a<x0/>bあい");
    expect(area.commitComposition("あい")).toBe(true);
    expect(area.isComposing()).toBe(false);
    expect(area.getOmDocument().textBeingComposed).toBe(false);

    area.setSelection(0, 1);
    expect(area.beginComposition()).toBe(true);
    expect(area.updateComposition("X")).toBe(true);
    expect(area.getText()).toBe("X<x0/>bあい");
    expect(area.cancelComposition()).toBe(true);
    expect(area.getText()).toBe("a<x0/>bあい");
  });

  it("EditorTextArea3 models directional selection, clipboard, token and focus events", () => {
    const area = new EditorTextArea3("source", "one \u200etwo <x0/> 😀");
    const end = area.getOmDocument().translationEnd;
    area.setSelection(end, 0);
    expect(area.getSelectionDirection()).toBe("backward");
    expect(area.getSelectedText()).toBe("one two <x0/> 😀");

    area.setCaretPosition(area.getText().indexOf("two") + "two".length);
    expect(area.deleteToken(-1)).toBe(true);
    expect(area.getText()).toBe("one \u200e <x0/> 😀");

    const tag = area.getText().indexOf("<x0/>");
    expect(area.selectTagAt(tag + 2)).toBe(true);
    expect(area.getSelectedText()).toBe("<x0/>");
    expect(area.pasteText("<x1/>")).toBe(true);
    expect(area.getText()).toBe("one \u200e <x1/> 😀");

    const events: boolean[] = [];
    area.onFocusChanged((focused) => events.push(focused));
    area.focus();
    area.focus();
    area.blur();
    expect(events).toEqual([true, false]);
  });

  it("EditorTextArea3 overtype replaces the inserted UTF-16 width", () => {
    const area = new EditorTextArea3("source", "abcde");
    area.setCaretPosition(1);
    area.toggleOvertype();
    expect(area.insertText("XY")).toBe(true);
    expect(area.getText()).toBe("aXYde");
    area.setCaretPosition(area.getOmDocument().translationEnd);
    expect(area.insertText("!")).toBe(true);
    expect(area.getText()).toBe("aXYde!");
  });

  it("EditorTextArea3 routes native beforeinput and pixel offsets through Document3", () => {
    const area = new EditorTextArea3("source", "a<x0/>bc");
    area.setCaretFromRenderedOffset(3, "before");
    expect(area.getCaretPosition()).toBe(1);
    area.setCaretFromRenderedOffset(3, "after");
    expect(area.getCaretPosition()).toBe(6);

    expect(area.handleBeforeInput("insertText", "😀")).toBe(true);
    expect(area.getText()).toBe("a<x0/>😀bc");
    expect(area.handleBeforeInput("deleteContentBackward")).toBe(true);
    expect(area.getText()).toBe("a<x0/>bc");
    expect(area.handleBeforeInput("formatBold")).toBe(false);

    area.setCaretPosition(area.getOmDocument().translationEnd);
    area.setCaretFromRenderedOffset(3, "before", true);
    expect({
      anchor: area.getSelectionAnchor(),
      focus: area.getSelectionFocus(),
      direction: area.getSelectionDirection(),
    }).toEqual({ anchor: 8, focus: 1, direction: "backward" });
  });

  it("EditorController synchronizes document, navigation, filter, history and undo", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "a.txt", source: "one", translation: "", note: "" },
      { file: "a.txt", source: "two", translation: "deux", note: "review" },
      { file: "b.txt", source: "three", translation: "", note: "" },
    ]);
    expect(controller.getCurrentEntryNumber()).toBe(1);
    controller.replaceEditText("un");
    expect(controller.entries[0]!.translation).toBe("un");
    expect(controller.undoEdit()).toBe("");
    expect(controller.redoEdit()).toBe("un");

    expect(controller.nextEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(2);
    expect(controller.gotoFile("b.txt")).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(3);
    expect(controller.gotoHistoryBack()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(2);
    expect(controller.gotoHistoryForward()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(3);

    controller.setFilter(makeFilter("noted"));
    expect(controller.getCurrentEntryNumber()).toBe(2);
    expect(controller.nextEntry()).toBe(false);
    expect(controller.getLoadedRange()).toEqual({ first: 1, last: 1 });
    controller.removeFilter();
    expect(controller.gotoEntry(1)).toBe(true);
    expect(controller.getCurrentTranslation()).toBe("un");
  });

  it("EditorController pages visible segments and refreshes marker spans", () => {
    const controller = new EditorController();
    controller.setPageRadius(1);
    controller.loadProject(
      [
        "one",
        "two",
        "three",
        "four\u00a0marked",
        "five",
        "six",
      ].map((translation, index) => ({
        file: index < 3 ? "a.txt" : "b.txt",
        source: `source ${index + 1}`,
        translation,
      })),
      4,
    );
    const page = controller.getLoadedPage();
    expect(page.map((entry) => entry.entryNumber)).toEqual([3, 4, 5]);
    expect(page.map((entry) => entry.active)).toEqual([false, true, false]);
    expect(page[1]!.marks.some((mark) => mark.painter === "nbsp")).toBe(true);
    expect(controller.getOmDocument()!.spans.some((span) => span.style === "marker:nbsp")).toBe(true);

    const generation = controller.markerSnapshot!.generation;
    controller.replaceEditText("four marked");
    expect(controller.markerSnapshot!.generation).toBeGreaterThan(generation);
    expect(controller.markerSnapshot!.marks.some((mark) => mark.painter === "nbsp")).toBe(false);
    expect(controller.getOmDocument()!.spans.some((span) => span.style === "marker:nbsp")).toBe(false);

    expect(controller.loadUp(2)).toBe(2);
    expect(controller.loadDown(2)).toBe(1);
    expect(controller.getLoadedPage().map((entry) => entry.entryNumber)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(controller.hasMoreBefore()).toBe(false);
    expect(controller.hasMoreAfter()).toBe(false);
  });

  it("EditorController restores a variable-height stable scroll anchor after prepend", () => {
    const controller = new EditorController();
    const anchor = controller.captureScrollAnchor(100, [
      { key: "first", top: 40, bottom: 90 },
      { key: "second", top: 90, bottom: 150 },
      { key: "third", top: 150, bottom: 220 },
    ]);
    expect(anchor).toEqual({ key: "second", offset: -10 });
    expect(controller.scrollAdjustmentForAnchor(anchor, 100, [
      { key: "prepended", top: 35, bottom: 170 },
      { key: "second", top: 225, bottom: 285 },
      { key: "third", top: 285, bottom: 355 },
    ])).toBe(135);
    expect(controller.scrollAdjustmentForAnchor(anchor, 100, [])).toBe(0);
  });

  it("EditorController synchronizes immutable renderer snapshots into stable segment pages", () => {
    const controller = new EditorController();
    controller.setPageRadius(1);
    const entries = [
      { file: "a.txt", id: "first", source: "one", translation: "un" },
      { file: "a.txt", id: "second", source: "two", translation: "deux" },
      { file: "b.txt", id: "third", source: "three", translation: "trois" },
    ];
    const page = controller.synchronizeRendererProject(
      entries,
      1,
      createDocument3("two", "DEUX"),
    );
    expect(page.map(({ key, entryNumber, translation, active }) => ({
      key,
      entryNumber,
      translation,
      active,
    }))).toEqual([
      { key: "0:a.txt:first", entryNumber: 1, translation: "un", active: false },
      { key: "1:a.txt:second", entryNumber: 2, translation: "DEUX", active: true },
      { key: "2:b.txt:third", entryNumber: 3, translation: "trois", active: false },
    ]);
  });
});
