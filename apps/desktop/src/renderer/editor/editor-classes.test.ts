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
    expect(controller.getLoadedRange()).toEqual({ first: 0, last: 2 });
    controller.removeFilter();
    expect(controller.gotoEntry(1)).toBe(true);
    expect(controller.getCurrentTranslation()).toBe("un");
  });
});
