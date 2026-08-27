import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
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
import {
  nextUntranslatedEntryIndex,
  selectionAfterEntryChange,
} from "./EditorSelection";
import { EditorTextArea3 } from "./EditorTextArea3";
import { HistoryPredictor } from "./history/HistoryPredictor";
import { HistoryCompleter } from "./history/HistoryCompleter";
import { bindMarkerRemark, IEditor } from "./IEditor";
import { makeFilter } from "./IEditorFilter";
import { buildActiveDocument } from "./SegmentBuilder";
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
    const here = dirname(fileURLToPath(import.meta.url));
    const golden = JSON.parse(readFileSync(
      join(here, "../../../../../fixtures/goldens/engine/ieditor_methods.json"),
      "utf8",
    )) as { methods: string[] };
    expect(Object.keys(IEditor).sort()).toEqual([...golden.methods].sort());
  });

  it("EditorController writes through IEditor", () => {
    const c = new EditorController();
    c.replaceEditText("hello");
    expect(c.getCurrentTranslation()).toBe("hello");
  });

  it("IEditor routes one-marker refreshes into the live marker controller", () => {
    const names: string[] = [];
    const unbind = bindMarkerRemark((name) => names.push(name));
    IEditor.remarkOneMarker("org.example.PluginMarker");
    unbind();
    IEditor.remarkOneMarker("org.example.DetachedMarker");
    expect(names).toEqual(["org.example.PluginMarker"]);
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

    area.setCaretPosition(area.getOmDocument().translationEnd);
    expect(area.beginComposition()).toBe(true);
    expect(area.handleBeforeInput("insertCompositionText", "日")).toBe(true);
    expect(area.handleBeforeInput("insertFromComposition", "日本語")).toBe(true);
    expect(area.getText()).toBe("a<x0/>bあい日本語");
    expect(area.isComposing()).toBe(false);
    expect(area.getOmDocument().textBeingComposed).toBe(false);

    expect(area.beginComposition()).toBe(true);
    expect(area.handleBeforeInput("insertCompositionText", "한")).toBe(true);
    expect(area.handleBeforeInput("insertText", "한국어")).toBe(true);
    expect(area.getText()).toBe("a<x0/>bあい日本語한국어");
    expect(area.isComposing()).toBe(false);

    area.focus();
    expect(area.beginComposition()).toBe(true);
    expect(area.updateComposition("失焦")).toBe(true);
    area.blur();
    expect({
      text: area.getText(),
      composing: area.isComposing(),
      documentComposing: area.getOmDocument().textBeingComposed,
      focused: area.hasFocus(),
    }).toEqual({
      text: "a<x0/>bあい日本語한국어失焦",
      composing: false,
      documentComposing: false,
      focused: false,
    });
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

  it("EditorTextArea3 keeps native mouse dragging on the Document3 UTF-16 path", () => {
    const area = new EditorTextArea3();
    const doc = buildActiveDocument(7, "selection source", "alpha 😀 beta");
    area.setDocument(doc);

    expect(area.beginMouseSelection(0, "before")).toBe(doc.translationStart);
    expect(area.isMouseSelecting()).toBe(true);
    expect(area.updateMouseSelection(5, "after")).toBe(true);
    expect({
      anchor: area.getSelectionAnchor(),
      focus: area.getSelectionFocus(),
      direction: area.getSelectionDirection(),
      selected: area.getSelectedText(),
    }).toEqual({
      anchor: doc.translationStart,
      focus: doc.translationStart + 5,
      direction: "forward",
      selected: "alpha",
    });
    expect(area.endMouseSelection(5, "after")).toBe(true);
    expect(area.isMouseSelecting()).toBe(false);

    expect(area.handleBeforeInput("insertText", "日本語")).toBe(true);
    expect(extractTranslation(area.getOmDocument())).toBe("日本語 😀 beta");
    expect(area.getOmDocument().dirty).toBe(true);
  });

  it("EditorTextArea3 hit-tests arbitrary protected parts and expands bidi selection", () => {
    const area = new EditorTextArea3("source %s", "a\u200e%s\u200fb");
    area.setProtectedRanges([{ start: 2, end: 4, tooltip: "printf %s" }]);

    area.setCaretFromRenderedOffset(3, "before");
    expect(area.getCaretPosition()).toBe(2);
    area.setCaretFromRenderedOffset(3, "after");
    expect(area.getCaretPosition()).toBe(4);
    expect(area.getProtectedTooltipAt(3)).toBe("printf %s");

    expect(area.selectProtectedPartAt(3)).toBe(true);
    expect({
      selected: area.getSelectedText(),
      anchor: area.getSelectionAnchor(),
      focus: area.getSelectionFocus(),
    }).toEqual({
      selected: "%s",
      anchor: 1,
      focus: 5,
    });
    expect(area.replaceSelection("X")).toBe(true);
    expect(area.getText()).toBe("aXb");

    const deletion = new EditorTextArea3("source %s", "a%s b");
    deletion.setProtectedRanges([{ start: 1, end: 3 }]);
    deletion.setCaretPosition(3);
    expect(deletion.deleteBackward()).toBe(true);
    expect(deletion.getText()).toBe("a b");
  });

  it("EditorController inserts at its relative selection through Document3", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "editor.txt", source: "source", translation: "alpha 😀 beta" },
    ]);
    expect(controller.getOmDocument()?.translationStart).toBe(7);

    controller.setCaretPosition({ selectionStart: 0, selectionEnd: 5 });
    expect(controller.getCurrentPositionInEntryTranslationInEditor()).toEqual({
      selectionStart: 0,
      selectionEnd: 5,
    });
    expect(controller.getSelectedText()).toBe("alpha");

    controller.insertText("日本語");
    expect({
      translation: controller.getCurrentTranslation(),
      entryTranslation: controller.entries[0]?.translation,
      position: controller.getCurrentPositionInEntryTranslationInEditor(),
      dirty: controller.getOmDocument()?.dirty,
    }).toEqual({
      translation: "日本語 😀 beta",
      entryTranslation: "日本語 😀 beta",
      position: { position: 3 },
      dirty: true,
    });

    expect(controller.undoEdit()).toBe("alpha 😀 beta");
    controller.setCaretPosition({ position: 6 });
    controller.insertText("X");
    expect(controller.getCurrentTranslation()).toBe("alpha X😀 beta");
  });

  it("EditorController replaces a UTF-16 translation range through EditorTextArea3", () => {
    const controller = new EditorController();
    controller.loadProject([{
      file: "editor.txt",
      source: "source",
      translation: "A😀B<x0/>C",
    }]);

    expect(controller.replacePartOfText("界", 1, 3)).toBe(true);
    expect({
      translation: controller.getCurrentTranslation(),
      entryTranslation: controller.entries[0]?.translation,
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      dirty: controller.getOmDocument()?.dirty,
    }).toEqual({
      translation: "A界B<x0/>C",
      entryTranslation: "A界B<x0/>C",
      caret: { position: 2 },
      dirty: true,
    });
    expect(() => controller.replacePartOfText("bad", -1, 2)).toThrow(
      "translation range -1..2 outside 0..9",
    );
    expect(controller.getCurrentTranslation()).toBe("A界B<x0/>C");

    expect(controller.undoEdit()).toBe("A😀B<x0/>C");
    expect(controller.getCurrentPositionInEntryTranslationInEditor()).toEqual({
      selectionStart: 1,
      selectionEnd: 3,
    });
  });

  it("EditorController changeCase clamps UTF-16 selections and leaves tags intact", () => {
    const controller = new EditorController();
    controller.loadProject([{
      file: "editor.txt",
      source: "source",
      translation: "one <x0/> lower case only 😀",
    }]);

    controller.setCaretPosition({ selectionStart: -20, selectionEnd: 200 });
    expect(controller.changeCase("upper")).toBe(true);
    expect(controller.getCurrentTranslation()).toBe("ONE <x0/> LOWER CASE ONLY 😀");
    expect(controller.getCurrentPositionInEntryTranslationInEditor()).toEqual({
      selectionStart: 0,
      selectionEnd: "ONE <x0/> LOWER CASE ONLY 😀".length,
    });

    controller.setCaretPosition({ position: "ONE <x0/> LOWER ca".length });
    expect(controller.changeCase("lower")).toBe(true);
    expect(controller.getCurrentTranslation()).toBe("ONE <x0/> LOWER case ONLY 😀");
    expect(controller.getSelectedText()).toBe("case");
    expect(controller.undoEdit()).toBe("ONE <x0/> LOWER CASE ONLY 😀");
  });

  it("EditorController refreshes an externally fixed entry without committing a stale draft", () => {
    const controller = new EditorController();
    controller.loadProject([{
      file: "editor.txt",
      source: "source",
      translation: "before",
    }]);
    controller.textArea.setText("stale local edit");
    controller.entries[0]!.translation = "fixed externally";

    expect(controller.refreshViewAfterFix([1])).toBe(true);
    expect({
      translation: controller.getCurrentTranslation(),
      stored: controller.entries[0]!.translation,
      dirty: controller.getOmDocument()!.dirty,
      editMode: controller.getOmDocument()!.editMode,
    }).toEqual({
      translation: "fixed externally",
      stored: "fixed externally",
      dirty: false,
      editMode: true,
    });
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

  it("EditorController rebuilds filters around the current entry and restores an empty view", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "a.txt", source: "first", translation: "", note: "earlier" },
      { file: "a.txt", source: "second", translation: "" },
      { file: "a.txt", source: "third", translation: "trois" },
      { file: "b.txt", source: "fourth", translation: "", note: "later" },
    ], 3);
    controller.setCaretPosition({ position: 2 });

    controller.setFilter(makeFilter("noted"));
    expect({
      active: controller.getCurrentEntryNumber(),
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      loaded: controller.getLoadedRange(),
    }).toEqual({
      active: 4,
      caret: { position: 0 },
      loaded: { first: 0, last: 3 },
    });

    controller.setFilter(makeFilter("search", "not present"));
    expect({
      retainedEntries: controller.entries.length,
      active: controller.getCurrentEntryNumber(),
      document: controller.getOmDocument(),
      loaded: controller.getLoadedRange(),
    }).toEqual({
      retainedEntries: 4,
      active: 0,
      document: null,
      loaded: { first: -1, last: -1 },
    });

    controller.removeFilter();
    expect({
      active: controller.getCurrentEntryNumber(),
      source: controller.getCurrentEntry()?.source,
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
    }).toEqual({
      active: 4,
      source: "fourth",
      caret: { position: 0 },
    });
  });

  it("EditorController reload rebinds a complete EntryKey and clamps its relative selection", () => {
    const controller = new EditorController();
    const firstKey = {
      file: "same.txt",
      source_text: "same",
      id: "duplicate",
      prev: "",
      next: "other",
      path: "/first",
    };
    const secondKey = {
      file: "same.txt",
      source_text: "same",
      id: "duplicate",
      prev: "other",
      next: "",
      path: "/second",
    };
    controller.loadProject([
      { key: firstKey, file: "same.txt", source: "same", translation: "first" },
      { key: secondKey, file: "same.txt", source: "same", translation: "abcdefghij" },
    ], 2);
    controller.setCaretPosition({ selectionStart: 2, selectionEnd: 8 });

    const rebound = controller.reloadProject([
      { key: secondKey, file: "same.txt", source: "same", translation: "xy" },
      { key: firstKey, file: "same.txt", source: "same", translation: "first reloaded" },
    ]);

    expect({
      rebound,
      active: controller.getCurrentEntryNumber(),
      key: controller.getCurrentEntry()?.key,
      translation: controller.getCurrentTranslation(),
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      dirty: controller.getOmDocument()?.dirty,
    }).toEqual({
      rebound: true,
      active: 1,
      key: secondKey,
      translation: "xy",
      caret: { position: 2 },
      dirty: false,
    });
  });

  it("EditorController commits live edits and applies cyclic Java navigation state", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "a.txt", source: "one", translation: "alpha", unique: true },
      {
        file: "a.txt",
        source: "two",
        translation: "beta",
        note: "review",
        unique: true,
        linked: "xAUTO",
      },
      {
        file: "b.txt",
        source: "three",
        translation: "",
        unique: false,
        linked: "xENFORCED",
      },
      { file: "b.txt", source: "four", translation: "delta", unique: true },
    ]);

    const live = controller.textArea;
    live.setCaretPosition(live.getOmDocument().translationEnd);
    expect(live.beginComposition()).toBe(true);
    expect(live.updateComposition("未提交")).toBe(true);
    expect(controller.nextEntry()).toBe(true);
    expect({
      saved: controller.entries[0]!.translation,
      active: controller.getCurrentEntryNumber(),
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      composing: live.isComposing(),
    }).toEqual({
      saved: "alpha未提交",
      active: 2,
      caret: { position: 0 },
      composing: false,
    });

    expect(controller.gotoEntry(1)).toBe(true);
    controller.setCaretPosition({ position: 3 });
    controller.commitAndLeave();
    expect({
      active: controller.getCurrentEntryNumber(),
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      editMode: controller.getOmDocument()!.editMode,
      dirty: controller.getOmDocument()!.dirty,
    }).toEqual({
      active: 1,
      caret: { position: 3 },
      editMode: true,
      dirty: false,
    });

    expect(controller.prevEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(4);
    expect(controller.nextTranslatedEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(1);
    expect(controller.nextEntryWithNote()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(2);
    expect(controller.nextXEnforcedEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(3);
    expect(controller.prevXAutoEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(2);
    expect(controller.nextUniqueEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(4);
    expect(controller.nextUntranslatedEntry()).toBe(true);
    expect(controller.getCurrentEntryNumber()).toBe(3);
  });

  it("EditorController propagates defaults while alternatives remain occurrence-scoped", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "a.txt", id: "first", source: "same", translation: "old" },
      { file: "a.txt", id: "second", source: "same", translation: "old" },
      {
        file: "b.txt",
        id: "third",
        source: "same",
        translation: "private third",
        isAlt: true,
      },
    ]);

    controller.replaceEditText("shared");
    expect(controller.entries.map(({ translation }) => translation)).toEqual([
      "shared",
      "old",
      "private third",
    ]);
    controller.commitAndLeave();
    expect(
      controller.entries.map(({ translation, isAlt }) => ({
        translation,
        isAlt: Boolean(isAlt),
      })),
    ).toEqual([
      { translation: "shared", isAlt: false },
      { translation: "shared", isAlt: false },
      { translation: "private third", isAlt: true },
    ]);

    expect(controller.gotoEntry(2)).toBe(true);
    controller.setCurrentTranslationVariant(false);
    controller.replaceEditText("private second");
    controller.commitAndLeave();
    expect(
      controller.entries.map(({ translation, isAlt }) => ({
        translation,
        isAlt: Boolean(isAlt),
      })),
    ).toEqual([
      { translation: "shared", isAlt: false },
      { translation: "private second", isAlt: true },
      { translation: "private third", isAlt: true },
    ]);

    controller.setCurrentTranslationVariant(true);
    controller.replaceEditText("new shared");
    controller.commitAndLeave();
    expect(
      controller.entries.map(({ translation, isAlt }) => ({
        translation,
        isAlt: Boolean(isAlt),
      })),
    ).toEqual([
      { translation: "new shared", isAlt: false },
      { translation: "new shared", isAlt: false },
      { translation: "private third", isAlt: true },
    ]);
  });

  it("EditorController undo restores selection and marker intervals together", () => {
    const controller = new EditorController();
    controller.loadProject([
      { file: "editor.txt", source: "source", translation: "a\u00a0b" },
    ]);
    controller.setCaretPosition({ selectionStart: 1, selectionEnd: 2 });
    expect(controller.markerSnapshot!.marks.some((mark) => mark.painter === "nbsp")).toBe(true);

    controller.insertText(" ");
    expect({
      translation: controller.getCurrentTranslation(),
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      nbsp: controller.markerSnapshot!.marks.some((mark) => mark.painter === "nbsp"),
    }).toEqual({
      translation: "a b",
      caret: { position: 2 },
      nbsp: false,
    });

    expect(controller.undoEdit()).toBe("a\u00a0b");
    expect({
      caret: controller.getCurrentPositionInEntryTranslationInEditor(),
      selected: controller.getSelectedText(),
      nbsp: controller.markerSnapshot!.marks.some((mark) => mark.painter === "nbsp"),
    }).toEqual({
      caret: { selectionStart: 1, selectionEnd: 2 },
      selected: "\u00a0",
      nbsp: true,
    });
    expect(controller.redoEdit()).toBe("a b");
    expect(controller.getCurrentPositionInEntryTranslationInEditor()).toEqual({ position: 2 });
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

  it("EditorController handles project/file drops and scopes leave issues to the old file", async () => {
    const controller = new EditorController();
    const actions: string[] = [];
    expect(await controller.handleFileDrop(
      { kind: "project", root: "/tmp/project" },
      false,
      {
        openProject: (root) => {
          actions.push(`open:${root}`);
        },
        importFiles: (paths) => {
          actions.push(`import:${paths.join(",")}`);
        },
      },
    )).toEqual({
      accepted: true,
      action: "open-project",
      paths: ["/tmp/project"],
    });
    expect(await controller.handleFileDrop(
      { kind: "files", paths: ["/tmp/a.txt", "/tmp/b.po"] },
      true,
      {
        openProject: (root) => {
          actions.push(`open:${root}`);
        },
        importFiles: (paths) => {
          actions.push(`import:${paths.join(",")}`);
        },
      },
    )).toEqual({
      accepted: true,
      action: "import-files",
      paths: ["/tmp/a.txt", "/tmp/b.po"],
    });
    expect(await controller.handleFileDrop(
      { kind: "files", paths: ["/tmp/rejected.txt"] },
      false,
      {
        openProject: () => undefined,
        importFiles: () => {
          throw new Error("must not import without a project");
        },
      },
    )).toEqual({
      accepted: false,
      action: "none",
      paths: ["/tmp/rejected.txt"],
    });
    expect(actions).toEqual([
      "open:/tmp/project",
      "import:/tmp/a.txt,/tmp/b.po",
    ]);

    const current = { file: "a.txt", source: "source", translation: "target" };
    const issues = [
      { kind: "tag", index: 0, file: "a.txt", message: "missing", severity: "error" },
      { kind: "tag", index: 3, file: "a.txt", message: "order", severity: "warn" },
      { kind: "spell", index: 1, file: "b.txt", message: "word", severity: "info" },
    ];
    expect(controller.checkIssuesOnLeave(current, 0, issues)).toEqual(issues.slice(0, 2));
    expect(controller.checkIssuesOnLeave(current, 0, issues, false)).toEqual([]);
  });

  it("EditorController synchronizes immutable renderer snapshots into stable segment pages", () => {
    const controller = new EditorController();
    controller.setPageRadius(1);
    const keys = [
      { file: "a.txt", source_text: "one", id: "first", prev: "", next: "two", path: null },
      { file: "a.txt", source_text: "two", id: "second", prev: "one", next: "", path: null },
      { file: "b.txt", source_text: "three", id: "third", prev: "", next: "", path: "/third" },
    ];
    const entries = [
      { key: keys[0], file: "a.txt", id: "first", source: "one", translation: "un" },
      { key: keys[1], file: "a.txt", id: "second", source: "two", translation: "deux" },
      { key: keys[2], file: "b.txt", id: "third", source: "three", translation: "trois" },
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
      { key: JSON.stringify(keys[0]), entryNumber: 1, translation: "un", active: false },
      { key: JSON.stringify(keys[1]), entryNumber: 2, translation: "DEUX", active: true },
      { key: JSON.stringify(keys[2]), entryNumber: 3, translation: "trois", active: false },
    ]);
    expect({
      controllerEntries: controller.entries,
      controllerDocument: controller.getOmDocument(),
      controllerEntryNumber: controller.getCurrentEntryNumber(),
      controllerSelection: controller.getCurrentPositionInEntryTranslationInEditor(),
    }).toEqual({
      controllerEntries: [],
      controllerDocument: null,
      controllerEntryNumber: 0,
      controllerSelection: { position: -1 },
    });
  });

  it("filters renderer pages and preserves caret only for the same complete EntryKey", () => {
    const controller = new EditorController();
    controller.setPageRadius(4);
    const entries = [
      { file: "a.txt", id: "one", source: "one", translation: "" },
      { file: "a.txt", id: "two", source: "two", translation: "deux" },
      { file: "b.txt", id: "three", source: "three", translation: "" },
    ];
    const page = controller.synchronizeRendererProject(
      entries,
      0,
      createDocument3("one", ""),
      makeFilter("untranslated"),
    );
    expect(page.map(({ entryNumber, source, active }) => ({
      entryNumber,
      source,
      active,
    }))).toEqual([
      { entryNumber: 1, source: "one", active: true },
      { entryNumber: 3, source: "three", active: false },
    ]);

    const key = JSON.stringify({
      file: "a.txt",
      source_text: "one",
      id: "one",
      prev: "",
      next: "",
      path: null,
    });
    expect(
      selectionAfterEntryChange(key, key, { anchor: 3, focus: 8 }, 5),
    ).toEqual({ anchor: 3, focus: 5 });
    expect(
      selectionAfterEntryChange(key, `${key}-other`, { anchor: 3, focus: 4 }, 7),
    ).toEqual({ anchor: 7, focus: 7 });
    expect(nextUntranslatedEntryIndex(entries, 1)).toBe(2);
    expect(nextUntranslatedEntryIndex(entries, 2)).toBe(0);
  });
});
