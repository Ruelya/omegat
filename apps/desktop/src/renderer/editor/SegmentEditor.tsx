import {
  useEffect,
  useRef,
  useState,
  type CompositionEvent,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { decorateText, deleteBackwardAtomic, deleteForwardAtomic, parseDocument } from "../lib/editor-doc";
import { t } from "../i18n";
import { useApp } from "../store/app";
import { DocumentFilter3, isPossible } from "./DocumentFilter3";
import {
  applyDocumentEdit,
  createDocument3,
  type Document3State,
} from "./Document3";
import { EditorController } from "./EditorController";
import { editorPopups } from "./EditorPopups";
import { EditorTextArea3 } from "./EditorTextArea3";

const filter3 = new DocumentFilter3();
const editorController = new EditorController();

function applyThroughDocument3(doc: Document3State, offset: number, length: number, text: string): Document3State {
  const result = filter3.replace(
    {
      text: doc.fullText || doc.translation,
      editMode: doc.editMode,
      trustedChangesInProgress: doc.trustedChangesInProgress,
      translationStart: doc.translationStart,
      translationEnd: doc.translationEnd,
      textBeingComposed: doc.textBeingComposed,
      allowTagEditing: !doc.tagsAtomic,
    },
    offset,
    length,
    text,
  );
  if (!result.applied) return doc;
  return applyDocumentEdit(doc, offset, length, text, {
    composed: result.doc.textBeingComposed,
  });
}

export function SegmentSource() {
  const e = useApp((s) => s.entries[s.index]);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  const selected = useApp((s) => s.selectedText);
  if (!e || !marks.displaySource) return null;
  const terms = glossary.map((g) => g.source);
  return (
    <div className={`src ${sourceClass(e, marks)} ${selected === e.source ? "is-selected-source" : ""}`}>
      {parseDocument(e.source).map((tok, i) =>
        tok.kind === "tag" ? (
          <span key={i} className="tag" data-tag={tok.value}>
            {tok.value}
          </span>
        ) : (
          decorateText(tok.value, marks, terms).map((sp, j) => (
            <span key={`${i}-${j}`} className={sp.cls.join(" ")}>
              {sp.text}
            </span>
          ))
        ),
      )}
    </div>
  );
}

function sourceClass(e: { translated: boolean; note: string; default_translation: boolean; properties: [string, string][] }, marks: ReturnType<typeof useApp.getState>["marks"]) {
  const cls = ["seg-source"];
  if (marks.translated && e.translated) cls.push("is-translated");
  if (marks.untranslated && !e.translated) cls.push("is-untranslated");
  if (marks.noted && e.note) cls.push("is-noted");
  if (marks.alternative && !e.default_translation) cls.push("is-alt");
  if (marks.autoPopulated && e.properties.some(([k]) => k === "tm")) cls.push("is-auto");
  return cls.join(" ");
}

export function SegmentEditor() {
  const document3 = useApp((s) => s.document3);
  const setDraft = useApp((s) => s.setDraft);
  const commit = useApp((s) => s.commit);
  const completer = useApp((s) => s.completer);
  const queryCompleter = useApp((s) => s.queryCompleter);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  const focus = useApp((s) => s.focusPanel);
  const tabAdvance = useApp((s) => Boolean(s.prefs?.tab_advance));
  const surface = useRef<HTMLDivElement>(null);
  const ime = useRef<HTMLTextAreaElement>(null);
  const interaction = useRef(new EditorTextArea3());
  const [caret, setCaret] = useState(document3.translation.length);
  const composing = useRef(false);
  editorController.document = document3;

  useEffect(() => {
    setCaret((c) => Math.min(c, document3.translation.length));
  }, [document3.translation]);

  useEffect(() => {
    if (focus === "editor") surface.current?.focus();
  }, [focus]);

  function applyDoc(next: Document3State, pos: number) {
    setDraft(next.translation);
    setCaret(pos);
    void queryCompleter(next.translation.slice(0, pos).split(/\s+/).pop() || "");
  }

  function insertAt(text: string) {
    const offset = document3.translationStart + caret;
    if (!isPossible(
      {
        text: document3.fullText,
        editMode: true,
        trustedChangesInProgress: false,
        translationStart: document3.translationStart,
        translationEnd: document3.translationEnd,
        textBeingComposed: composing.current,
        allowTagEditing: false,
      },
      offset,
      0,
    )) {
      return;
    }
    const next = applyThroughDocument3(document3, offset, 0, text);
    applyDoc(next, caret + text.length);
  }

  function beginComposition() {
    composing.current = true;
    interaction.current.setDocument(document3);
    interaction.current.setCaretPosition(document3.translationStart + caret);
    interaction.current.beginComposition();
  }

  function updateComposition(ev: CompositionEvent<HTMLDivElement>) {
    const area = interaction.current;
    if (!area.isComposing() || !area.updateComposition(ev.data)) return;
    const next = area.getOmDocument();
    applyDoc(next, area.getCaretPosition() - next.translationStart);
  }

  function finishComposition(ev: CompositionEvent<HTMLDivElement>) {
    const area = interaction.current;
    if (area.isComposing()) {
      area.commitComposition(ev.data);
      const next = area.getOmDocument();
      applyDoc(next, area.getCaretPosition() - next.translationStart);
    } else if (ev.data) {
      insertAt(ev.data);
    }
    composing.current = false;
  }

  function onKey(ev: KeyboardEvent<HTMLDivElement>) {
    if (composing.current) return;
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      void commit();
      return;
    }
    if (ev.key === "Tab" && tabAdvance) {
      ev.preventDefault();
      void commit();
      return;
    }
    if (ev.key === "Tab" && !tabAdvance && completer[0]) {
      ev.preventDefault();
      insertAt(completer[0].text);
      return;
    }
    if (ev.key === "Backspace") {
      ev.preventDefault();
      const next = deleteBackwardAtomic(document3.translation, caret);
      const removed = document3.translation.length - next.text.length;
      if (removed > 0) {
        const doc = applyThroughDocument3(
          document3,
          document3.translationStart + next.pos,
          removed,
          "",
        );
        applyDoc(doc, next.pos);
      }
      return;
    }
    if (ev.key === "Delete") {
      ev.preventDefault();
      const next = deleteForwardAtomic(document3.translation, caret);
      const removed = document3.translation.length - next.text.length;
      if (removed > 0) {
        const start = Math.min(caret, next.pos);
        const doc = applyThroughDocument3(
          document3,
          document3.translationStart + start,
          removed,
          "",
        );
        applyDoc(doc, next.pos);
      }
      return;
    }
    if (ev.key === "ArrowLeft") {
      ev.preventDefault();
      setCaret(Math.max(0, caret - 1));
      return;
    }
    if (ev.key === "ArrowRight") {
      ev.preventDefault();
      setCaret(Math.min(document3.translation.length, caret + 1));
      return;
    }
    if (ev.key === "Home") {
      ev.preventDefault();
      setCaret(0);
      return;
    }
    if (ev.key === "End") {
      ev.preventDefault();
      setCaret(document3.translation.length);
      return;
    }
    if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key.length === 1) {
      ev.preventDefault();
      insertAt(ev.key);
    }
  }

  function onClick(ev: MouseEvent<HTMLDivElement>) {
    const target = ev.target as HTMLElement;
    const tag = target.closest("[data-tag]") as HTMLElement | null;
    if (tag?.dataset.tag) {
      const idx = document3.translation.indexOf(tag.dataset.tag);
      if (idx >= 0) setCaret(idx + tag.dataset.tag.length);
      return;
    }
    const offset = target.dataset.offset;
    if (offset != null) setCaret(Number(offset));
  }

  const terms = glossary.map((g) => g.source);
  const draft = document3.translation;
  const before = draft.slice(0, caret);
  const after = draft.slice(caret);
  const popups = editorPopups();

  return (
    <div className="editor-doc">
      <div className="pane-h">{t("target")}</div>
      <div
        ref={surface}
        className="tgt editor-surface"
        tabIndex={0}
        role="textbox"
        aria-multiline="true"
        aria-label={t("target")}
        onKeyDown={onKey}
        onClick={onClick}
        onContextMenu={(ev) => {
          ev.preventDefault();
          useApp.getState().logLine(`editor popup: ${popups.map((p) => p.id).join(",")}`);
        }}
        onPaste={(ev) => {
          ev.preventDefault();
          const text = ev.clipboardData.getData("text/plain");
          if (text) insertAt(text);
        }}
        onCompositionStart={beginComposition}
        onCompositionUpdate={updateComposition}
        onCompositionEnd={finishComposition}
      >
        <span className="editor-before">
          {parseDocument(before).map((tok, i) =>
            tok.kind === "tag" ? (
              <span key={i} className="tag tag-protected" data-tag={tok.value}>{tok.value}</span>
            ) : (
              decorateText(tok.value, marks, terms).map((sp, j) => (
                <span key={`${i}-${j}`} className={sp.cls.join(" ")}>{sp.text}</span>
              ))
            ),
          )}
        </span>
        <span className="caret" aria-hidden />
        <span className="editor-after">
          {parseDocument(after).map((tok, i) =>
            tok.kind === "tag" ? (
              <span key={i} className="tag tag-protected" data-tag={tok.value}>{tok.value}</span>
            ) : (
              decorateText(tok.value, marks, terms).map((sp, j) => (
                <span key={`${i}-${j}`} className={sp.cls.join(" ")}>{sp.text}</span>
              ))
            ),
          )}
        </span>
        <textarea
          ref={ime}
          className="ime-proxy"
          aria-hidden
          tabIndex={-1}
          value=""
          onChange={() => undefined}
        />
      </div>
      {completer.length > 0 && (
        <div className="completer">
          {completer.slice(0, 8).map((c, i) => (
            <button
              key={`${c.kind}-${c.text}-${i}`}
              type="button"
              className="hit"
              onClick={() => insertAt(c.text)}
            >
              <span className="score">{c.kind}</span> {c.text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export { createDocument3 };
