import { describe, expect, it } from "vitest";
import { EditorController } from "../editor/EditorController";
import {
  CommentsController,
  DictionaryController,
  DockNotificationController,
  DockPopupController,
  entryComment,
  GlossaryController,
  LatestDockRequest,
  MachineTranslateController,
  MatchesController,
  MultipleTranslationsController,
  NotesController,
  SegmentPropertiesController,
  type MultipleTranslationTarget,
} from "./dock-controllers";
import type { EntryDto } from "./types";

function makeEntry(
  index: number,
  source: string,
  translation: string,
  partial: Partial<EntryDto> = {},
): EntryDto {
  const id = partial.id ?? `id-${index}`;
  return {
    index,
    key: partial.key ?? {
      file: partial.file ?? "same.txt",
      source_text: source,
      id,
      prev: index === 0 ? "" : `before-${index}`,
      next: index === 2 ? "" : `after-${index}`,
      path: `/unit/${index}`,
    },
    file: partial.file ?? "same.txt",
    id,
    source,
    translation,
    note: "",
    comment: "",
    default_translation: true,
    revision: 1,
    translated: translation.length > 0,
    tags: [],
    properties: [],
    ...partial,
  };
}

describe("Swing-depth desktop dock controllers", () => {
  it("sorts and selects fuzzy matches before applying them through EditorController", () => {
    const editor = new EditorController();
    editor.loadProject([{ file: "a.txt", source: "source", translation: "draft" }]);
    const controller = new MatchesController([
      { source: "low", translation: "LOW", score: 70, comes_from: "tm/low" },
      { source: "high", translation: "HIGH", score: 95, comes_from: "tm/high" },
    ]);

    expect(controller.matches.map(({ score }) => score)).toEqual([95, 70]);
    expect(controller.getActiveMatch()?.translation).toBe("HIGH");
    expect(controller.previous()).toBe(0);
    expect(controller.next()).toBe(1);
    expect(controller.next()).toBe(1);

    expect(controller.apply(editor, "overwrite", 0)).toBe(true);
    editor.setCaretPosition({ position: 2 });
    expect(controller.apply(editor, "insert", 1)).toBe(true);
    expect(editor.getCurrentTranslation()).toBe("HILOWGH");
  });

  it("renders and inserts glossary results through the live document selection", () => {
    const editor = new EditorController();
    editor.loadProject([{ file: "a.txt", source: "snowman", translation: "A B" }]);
    editor.setCaretPosition({ selectionStart: 2, selectionEnd: 3 });
    const controller = new GlossaryController([
      { source: "snowman", target: "sneeuwpop", comment: "https://example.test/a%20b" },
    ]);

    expect(controller.getText()).toBe(
      "snowman = sneeuwpop\n1. https://example.test/a b",
    );
    expect(controller.insertTarget(editor, 0)).toBe(true);
    expect(editor.getCurrentTranslation()).toBe("A sneeuwpop");
    expect(controller.insertTarget(editor, 4)).toBe(false);
  });

  it("keeps note undo/redo scoped to one activated entry", () => {
    const notes = new NotesController();
    notes.set("first");
    notes.set("second");
    expect(notes.undo()).toBe("first");
    expect(notes.undo()).toBeNull();
    expect(notes.redo()).toBe("first");
    notes.activate("other entry");
    expect(notes.undo()).toBe("other entry");
    notes.set("");
    expect(notes.get()).toBeNull();
    notes.clear();
    expect(notes.get()).toBeNull();
  });

  it("orders comment providers and renders complete entry metadata", () => {
    const entry = makeEntry(0, "source", "", {
      comment: "source comment",
      properties: [["translation", "source-side translation"]],
      key: {
        file: "a.txt",
        source_text: "source",
        id: "unit",
        prev: "",
        next: "",
        path: "group\\nitem",
      },
    });
    const comments = new CommentsController<EntryDto>();
    const late = () => "late";
    comments.addProvider(late, 100);
    comments.addProvider(entryComment, 0);

    expect(comments.render(entry)).toBe(
      "ID unit\nPath group\nitem\nTranslation source-side translation\n"
      + "Comment\nsource comment\nlate",
    );
    expect(comments.removeProvider(late)).toBe(true);
    expect(comments.removeProvider(late)).toBe(false);
    expect(comments.render(entry)).not.toContain("late");
  });

  it("keeps multiple translations key-scoped and can promote one through EditorController", () => {
    const entries = [
      makeEntry(0, "same", "shared"),
      makeEntry(1, "same", "private second", { default_translation: false }),
      makeEntry(2, "same", "private third", { default_translation: false }),
    ];
    const editor = new EditorController();
    editor.loadProject(entries.map((entry) => ({
      key: entry.key,
      file: entry.file,
      id: entry.id,
      source: entry.source,
      translation: entry.translation,
      translated: entry.translated,
      isAlt: !entry.default_translation,
    })));
    const controller = new MultipleTranslationsController(entries, 0);
    const target: MultipleTranslationTarget = {
      getCurrentTranslation: () => editor.getCurrentTranslation(),
      replaceEditText: (text) => editor.replaceEditText(text),
      insertText: (text) => editor.insertText(text),
      commitTranslationVariant: (isDefault) => {
        editor.setCurrentTranslationVariant(isDefault);
        editor.commitAndLeave();
      },
      gotoEntry: (source, key) => editor.gotoEntryBySourceAndKey(source, key),
    };

    expect(controller.rows).toHaveLength(3);
    expect(controller.goto(target, 1)).toBe(true);
    expect(editor.getCurrentEntry()?.key).toEqual(entries[1]!.key);
    expect(controller.makeDefault(target, 2)).toBe(true);
    expect(editor.getCurrentTranslation()).toBe("private third");
    expect(editor.getCurrentEntry()?.isAlt).toBe(false);
    expect(editor.entries[0]?.translation).toBe("private third");

    const singleton = new MultipleTranslationsController(
      [makeEntry(0, "only", "translation")],
      0,
    );
    expect(singleton.rows).toEqual([]);
  });

  it("sorts and cycles MT results before inserting the selected engine result", () => {
    const editor = new EditorController();
    editor.loadProject([{ file: "a.txt", source: "source", translation: "" }]);
    const mt = new MachineTranslateController([
      { engine: "zeta", text: "Z" },
      { engine: "alpha", text: "A" },
    ]);

    expect(mt.cycle()).toEqual({ engine: "alpha", text: "A" });
    expect(mt.apply(editor, "overwrite")).toBe(true);
    expect(editor.getCurrentTranslation()).toBe("A");
    expect(mt.cycle()).toEqual({ engine: "zeta", text: "Z" });
    expect(mt.apply(editor, "insert")).toBe(true);
    expect(editor.getCurrentTranslation()).toBe("AZ");
  });

  it("orders dictionary articles and focuses exact or stemmed words", () => {
    const dictionary = new DictionaryController([
      { word: "running", definition: "moving quickly", source: "b.dsl" },
      { word: "run", definition: "move quickly", source: "a.dsl" },
      { word: "run", definition: "a sequence", source: "b.dsl" },
    ]);

    expect(dictionary.entries.map(({ word, definition }) => [word, definition])).toEqual([
      ["run", "a sequence"],
      ["run", "move quickly"],
      ["running", "moving quickly"],
    ]);
    expect(dictionary.focusWord("RUN")).toBe(0);
    expect(dictionary.focusWord("runner", ["running"])).toBe(2);
    expect(dictionary.focusWord("missing")).toBe(-1);
  });

  it("builds segment property rows and exact notification indices", () => {
    const entry = makeEntry(0, "source", "target", {
      note: "review",
      comment: "developer comment",
      default_translation: false,
      revision: 7,
      properties: [["origin", "tm/legacy.tmx"], ["file", "must not replace key file"]],
    });
    const properties = new SegmentPropertiesController(["hasNote", "origin"]);
    const rows = properties.rows(entry);

    expect(rows.map(({ key, value }) => [key, value])).toEqual([
      ["hasComment", "yes"],
      ["file", "same.txt"],
      ["id", "id-0"],
      ["path", "/unit/0"],
      ["hasNote", "yes"],
      ["isAlt", "yes"],
      ["revision", "7"],
      ["origin", "tm/legacy.tmx"],
    ]);
    expect(properties.notifiedRowIndices(entry)).toEqual([4, 7]);
    properties.toggleNotification("file", true);
    properties.toggleNotification("hasNote", false);
    expect(properties.getNotificationKeys()).toEqual(["origin", "file"]);
    expect(properties.notifiedRowIndices(entry)).toEqual([1, 7]);
  });

  it("resolves source-only defaults and platform target paths without crossing alternatives", () => {
    const editor = new EditorController();
    const defaultKey = makeEntry(0, "same", "default").key;
    const altKey = makeEntry(1, "same", "alternative", { default_translation: false }).key;
    editor.loadProject([
      {
        key: altKey,
        file: "folder/source.txt",
        source: "same",
        translation: "alternative",
        isAlt: true,
      },
      {
        key: defaultKey,
        file: "folder/source.txt",
        source: "same",
        translation: "default",
      },
    ]);

    expect(editor.gotoEntryBySourceAndKey("same")).toBe(true);
    expect(editor.getCurrentEntry()?.key).toEqual(defaultKey);
    expect(editor.getCurrentTargetFile("/tmp/target/")).toBe("/tmp/target/folder/source.txt");
    expect(editor.gotoEntryBySourceAndKey("same", altKey)).toBe(true);
    expect(editor.getCurrentEntry()?.key).toEqual(altKey);
    expect(editor.getCurrentTargetFile("C:\\target\\")).toBe("C:\\target\\folder\\source.txt");
  });

  it("cancels stale asynchronous dock results before they can publish", async () => {
    let resolveOld!: (value: string) => void;
    const old = new Promise<string>((resolve) => {
      resolveOld = resolve;
    });
    const published: string[] = [];
    const requests = new LatestDockRequest<string>();

    const first = requests.run(() => old, (value) => published.push(value));
    expect(requests.isPending()).toBe(true);
    const second = requests.run(async () => "new", (value) => published.push(value));
    expect(await second).toBe(true);
    resolveOld("old");
    expect(await first).toBe(false);
    expect(requests.isPending()).toBe(false);
    expect(published).toEqual(["new"]);
  });

  it("dispatches hit/miss notifications and popup actions through dock controllers", () => {
    const notifications = new DockNotificationController(true, false);
    expect(notifications.signal(2)).toBe("hit");
    expect(notifications.signal(0)).toBeNull();
    notifications.setNotifyHits(false);
    notifications.setNotifyMisses(true);
    expect(notifications.getSettings()).toEqual({ hits: false, misses: true });
    expect(notifications.signal(2)).toBeNull();
    expect(notifications.signal(0)).toBe("miss");

    const invoked: string[] = [];
    const popup = new DockPopupController();
    popup.update([
      { id: "disabled", label: "Disabled", disabled: true, action: () => invoked.push("disabled") },
      { id: "insert", label: "Insert", checked: true, action: () => invoked.push("insert") },
    ]);
    expect(popup.open(-5, 12)).toMatchObject({ open: true, x: 0, y: 12 });
    expect(popup.invoke("disabled")).toBe(false);
    expect(popup.snapshot().open).toBe(true);
    expect(popup.invoke("insert")).toBe(true);
    expect(popup.snapshot().open).toBe(false);
    expect(invoked).toEqual(["insert"]);
  });
});
