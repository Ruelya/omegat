import { describe, expect, it } from "vitest";
import { AutoCompleter } from "./autocompleter/AutoCompleter";
import { GlossaryAutoCompleterView } from "./autocompleter/GlossaryAutoCompleterView";
import { AutotextAutoCompleterView } from "./autotext/AutotextAutoCompleterView";
import { commitAndDeactivate, createDocument3, insertText, replaceEditText } from "./Document3";
import { allowInsert } from "./DocumentFilter3";
import { EditorController } from "./EditorController";
import { HistoryPredictor } from "./history/HistoryPredictor";
import { HistoryCompleter } from "./history/HistoryCompleter";
import { IEditor } from "./IEditor";
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
});
