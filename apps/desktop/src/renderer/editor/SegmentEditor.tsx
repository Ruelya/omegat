import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import {
  decorateText,
  deleteBackwardAtomic,
  deleteForwardAtomic,
  insertAtomic,
  moveCaret,
  parseDocument,
  snapCaret,
} from "../lib/editor-doc";
import { t } from "../i18n";
import { useApp } from "../store/app";

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
  const draft = useApp((s) => s.draft);
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
  const [caret, setCaret] = useState(draft.length);
  const composing = useRef(false);

  useEffect(() => {
    setCaret((c) => snapCaret(draft, Math.min(c, draft.length)));
  }, [draft]);

  useEffect(() => {
    if (focus === "editor") surface.current?.focus();
  }, [focus]);

  function apply(next: { text: string; pos: number }) {
    setDraft(next.text);
    setCaret(next.pos);
    void queryCompleter(next.text.slice(0, next.pos).split(/\s+/).pop() || "");
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
      apply(insertAtomic(draft, caret, completer[0].text));
      return;
    }
    if (ev.key === "Backspace") {
      ev.preventDefault();
      apply(deleteBackwardAtomic(draft, caret));
      return;
    }
    if (ev.key === "Delete") {
      ev.preventDefault();
      apply(deleteForwardAtomic(draft, caret));
      return;
    }
    if (ev.key === "ArrowLeft") {
      ev.preventDefault();
      setCaret(moveCaret(draft, caret, -1));
      return;
    }
    if (ev.key === "ArrowRight") {
      ev.preventDefault();
      setCaret(moveCaret(draft, caret, 1));
      return;
    }
    if (ev.key === "Home") {
      ev.preventDefault();
      setCaret(0);
      return;
    }
    if (ev.key === "End") {
      ev.preventDefault();
      setCaret(draft.length);
      return;
    }
    if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key.length === 1) {
      ev.preventDefault();
      apply(insertAtomic(draft, caret, ev.key));
    }
  }

  function onClick(ev: MouseEvent<HTMLDivElement>) {
    const target = ev.target as HTMLElement;
    const tag = target.closest("[data-tag]") as HTMLElement | null;
    if (tag?.dataset.tag) {
      const idx = draft.indexOf(tag.dataset.tag);
      if (idx >= 0) setCaret(idx + tag.dataset.tag.length);
      return;
    }
    const offset = target.dataset.offset;
    if (offset != null) setCaret(Number(offset));
  }

  const terms = glossary.map((g) => g.source);
  const before = draft.slice(0, caret);
  const after = draft.slice(caret);

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
        onPaste={(ev) => {
          ev.preventDefault();
          const text = ev.clipboardData.getData("text/plain");
          if (text) apply(insertAtomic(draft, caret, text));
        }}
        onCompositionStart={() => {
          composing.current = true;
        }}
        onCompositionEnd={(ev) => {
          composing.current = false;
          if (ev.data) apply(insertAtomic(draft, caret, ev.data));
        }}
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
              onClick={() => apply(insertAtomic(useApp.getState().draft, useApp.getState().draft.length, c.text))}
            >
              <span className="score">{c.kind}</span> {c.text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
