import {
  useEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type CompositionEvent,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  type UIEvent,
} from "react";
import { decorateText, parseDocument } from "../lib/editor-doc";
import { t } from "../i18n";
import { useApp } from "../store/app";
import {
  createDocument3,
  extractTranslation,
  type Document3State,
} from "./Document3";
import { EditorController } from "./EditorController";
import { editorPopups } from "./EditorPopups";
import { EditorTextArea3 } from "./EditorTextArea3";

const editorController = new EditorController();

function renderRichText(
  text: string,
  offset: number,
  marks: ReturnType<typeof useApp.getState>["marks"],
  terms: string[],
  keyPrefix: string,
): ReactNode[] {
  let cursor = offset;
  return parseDocument(text).flatMap((tok, i) => {
    const start = cursor;
    cursor += tok.value.length;
    if (tok.kind === "tag") {
      return (
        <span
          key={`${keyPrefix}-tag-${i}`}
          className="tag tag-protected"
          data-tag={tok.value}
          data-offset={start}
        >
          {tok.value}
        </span>
      );
    }
    let textOffset = start;
    return decorateText(tok.value, marks, terms).map((span, j) => {
      const spanOffset = textOffset;
      textOffset += span.text.length;
      return (
        <span
          key={`${keyPrefix}-text-${i}-${j}`}
          className={span.cls.join(" ")}
          data-offset={spanOffset}
        >
          {span.text}
        </span>
      );
    });
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
  const entries = useApp((s) => s.entries);
  const activeIndex = useApp((s) => s.index);
  const document3 = useApp((s) => s.document3);
  const setDraft = useApp((s) => s.setDraft);
  const commit = useApp((s) => s.commit);
  const select = useApp((s) => s.select);
  const completer = useApp((s) => s.completer);
  const queryCompleter = useApp((s) => s.queryCompleter);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  const focus = useApp((s) => s.focusPanel);
  const tabAdvance = useApp((s) => Boolean(s.prefs?.tab_advance));
  const surface = useRef<HTMLDivElement>(null);
  const ime = useRef<HTMLTextAreaElement>(null);
  const interaction = useRef(new EditorTextArea3());
  const [selection, setSelection] = useState({
    anchor: document3.translation.length,
    focus: document3.translation.length,
  });
  const [pageRadius, setPageRadius] = useState(8);
  const composing = useRef(false);
  editorController.setPageRadius(pageRadius);
  const loadedPage = editorController.synchronizeRendererProject(entries, activeIndex, document3);

  useEffect(() => {
    setSelection((current) => ({
      anchor: Math.min(current.anchor, document3.translation.length),
      focus: Math.min(current.focus, document3.translation.length),
    }));
  }, [document3.translation]);

  useEffect(() => {
    const end = document3.translation.length;
    setSelection({ anchor: end, focus: end });
  }, [activeIndex]);

  useEffect(() => {
    if (focus === "editor") surface.current?.focus();
  }, [focus]);

  function prepareInteraction(): EditorTextArea3 {
    const area = interaction.current;
    area.setDocument(document3);
    area.setSelection(
      document3.translationStart + selection.anchor,
      document3.translationStart + selection.focus,
    );
    return area;
  }

  function readSelection(area: EditorTextArea3) {
    const start = area.getOmDocument().translationStart;
    setSelection({
      anchor: area.getSelectionAnchor() - start,
      focus: area.getSelectionFocus() - start,
    });
  }

  function applyDoc(next: Document3State, area: EditorTextArea3) {
    const translation = extractTranslation(next) ?? next.translation;
    setDraft(translation);
    readSelection(area);
    const pos = area.getSelectionFocus() - next.translationStart;
    void queryCompleter(translation.slice(0, pos).split(/\s+/).pop() || "");
  }

  function insertAt(text: string) {
    const area = prepareInteraction();
    if (!area.insertText(text)) return;
    applyDoc(area.getOmDocument(), area);
  }

  function beginComposition() {
    composing.current = true;
    prepareInteraction().beginComposition();
  }

  function updateComposition(ev: CompositionEvent<HTMLDivElement>) {
    const area = interaction.current;
    if (!area.isComposing() || !area.updateComposition(ev.data)) return;
    const next = area.getOmDocument();
    applyDoc(next, area);
  }

  function finishComposition(ev: CompositionEvent<HTMLDivElement>) {
    const area = interaction.current;
    if (area.isComposing()) {
      area.commitComposition(ev.data);
      const next = area.getOmDocument();
      applyDoc(next, area);
    } else if (ev.data) {
      insertAt(ev.data);
    }
    composing.current = false;
  }

  function onKey(ev: KeyboardEvent<HTMLDivElement>) {
    if (composing.current) {
      if (ev.key === "Escape" && interaction.current.cancelComposition()) {
        ev.preventDefault();
        composing.current = false;
        applyDoc(interaction.current.getOmDocument(), interaction.current);
      }
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "a") {
      ev.preventDefault();
      const area = prepareInteraction();
      area.selectAll();
      readSelection(area);
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key === "Backspace") {
      ev.preventDefault();
      const area = prepareInteraction();
      if (area.deleteToken(-1)) applyDoc(area.getOmDocument(), area);
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key === "Delete") {
      ev.preventDefault();
      const area = prepareInteraction();
      if (area.deleteToken(1)) applyDoc(area.getOmDocument(), area);
      return;
    }
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      void commit();
      return;
    }
    if (ev.key === "Enter" && ev.shiftKey) {
      ev.preventDefault();
      insertAt("\n");
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
      const area = prepareInteraction();
      if (area.deleteBackward()) applyDoc(area.getOmDocument(), area);
      return;
    }
    if (ev.key === "Delete") {
      ev.preventDefault();
      const area = prepareInteraction();
      if (area.deleteForward()) applyDoc(area.getOmDocument(), area);
      return;
    }
    if (ev.key === "ArrowLeft") {
      ev.preventDefault();
      const area = prepareInteraction();
      area.moveCaret(-1, ev.shiftKey);
      readSelection(area);
      return;
    }
    if (ev.key === "ArrowRight") {
      ev.preventDefault();
      const area = prepareInteraction();
      area.moveCaret(1, ev.shiftKey);
      readSelection(area);
      return;
    }
    if (ev.key === "Home") {
      ev.preventDefault();
      const area = prepareInteraction();
      if (ev.shiftKey) {
        area.setSelection(area.getSelectionAnchor(), document3.translationStart);
      } else {
        area.setCaretPosition(document3.translationStart);
      }
      readSelection(area);
      return;
    }
    if (ev.key === "End") {
      ev.preventDefault();
      const area = prepareInteraction();
      if (ev.shiftKey) {
        area.setSelection(area.getSelectionAnchor(), document3.translationEnd);
      } else {
        area.setCaretPosition(document3.translationEnd);
      }
      readSelection(area);
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
      const idx = Number(tag.dataset.offset);
      if (Number.isFinite(idx)) {
        setSelection({ anchor: idx + tag.dataset.tag.length, focus: idx + tag.dataset.tag.length });
      }
      return;
    }
    const offset = target.dataset.offset;
    if (offset != null) {
      const position = Number(offset);
      if (Number.isFinite(position)) setSelection({ anchor: position, focus: position });
    }
  }

  function onDoubleClick(ev: MouseEvent<HTMLDivElement>) {
    const target = (ev.target as HTMLElement).closest("[data-tag]") as HTMLElement | null;
    if (!target?.dataset.tag) return;
    const offset = Number(target.dataset.offset);
    if (!Number.isFinite(offset)) return;
    const area = prepareInteraction();
    if (area.selectTagAt(document3.translationStart + offset)) readSelection(area);
  }

  function onCopy(ev: ClipboardEvent<HTMLDivElement>) {
    const area = prepareInteraction();
    const text = area.getSelectedText();
    if (!text) return;
    ev.preventDefault();
    ev.clipboardData.setData("text/plain", text);
  }

  function onCut(ev: ClipboardEvent<HTMLDivElement>) {
    const area = prepareInteraction();
    const text = area.cutSelection();
    if (text == null) return;
    ev.preventDefault();
    ev.clipboardData.setData("text/plain", text);
    applyDoc(area.getOmDocument(), area);
  }

  function onEditorFocus() {
    interaction.current.focus();
    if (document.activeElement === surface.current) ime.current?.focus({ preventScroll: true });
  }

  function onEditorBlur(ev: FocusEvent<HTMLDivElement>) {
    if (ev.relatedTarget && ev.currentTarget.contains(ev.relatedTarget as Node)) return;
    interaction.current.blur();
  }

  function onPageScroll(ev: UIEvent<HTMLDivElement>) {
    const el = ev.currentTarget;
    const available = el.scrollHeight - el.clientHeight;
    if (available <= 0) return;
    const position = el.scrollTop / available;
    if (
      (position <= 0.2 && editorController.hasMoreBefore())
      || (position >= 0.8 && editorController.hasMoreAfter())
    ) {
      setPageRadius((radius) => Math.min(entries.length, radius + 8));
    }
  }

  const terms = glossary.map((g) => g.source);
  const draft = document3.translation;
  const selectionStart = Math.min(selection.anchor, selection.focus);
  const selectionEnd = Math.max(selection.anchor, selection.focus);
  const beforeSelection = draft.slice(0, selectionStart);
  const selected = draft.slice(selectionStart, selectionEnd);
  const afterSelection = draft.slice(selectionEnd);
  const popups = editorPopups();

  return (
    <div
      className="editor-doc"
      data-first-loaded={editorController.getLoadedRange().first}
      data-last-loaded={editorController.getLoadedRange().last}
      onScroll={onPageScroll}
    >
      {loadedPage.map((entry) => entry.active ? (
        <section className="editor-segment is-active" data-entry={entry.entryNumber} key={entry.key}>
          <div className="segment-meta">{entry.file} · #{entry.entryNumber}</div>
          <div className="pane-h">{t("source")}</div>
          <SegmentSource />
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
            onDoubleClick={onDoubleClick}
            onCopy={onCopy}
            onCut={onCut}
            onFocus={onEditorFocus}
            onBlur={onEditorBlur}
            onContextMenu={(ev) => {
              ev.preventDefault();
              useApp.getState().logLine(`editor popup: ${popups.map((p) => p.id).join(",")}`);
            }}
            onPaste={(ev) => {
              ev.preventDefault();
              const text = ev.clipboardData.getData("text/plain");
              if (text) {
                const area = prepareInteraction();
                if (area.pasteText(text)) applyDoc(area.getOmDocument(), area);
              }
            }}
            onCompositionStart={beginComposition}
            onCompositionUpdate={updateComposition}
            onCompositionEnd={finishComposition}
          >
            {renderRichText(beforeSelection, 0, marks, terms, "before")}
            {selected && selection.focus === selectionStart && <span className="caret" aria-hidden />}
            {selected && (
              <span className="editor-selection">
                {renderRichText(selected, selectionStart, marks, terms, "selection")}
              </span>
            )}
            {(!selected || selection.focus === selectionEnd) && <span className="caret" aria-hidden />}
            {renderRichText(afterSelection, selectionEnd, marks, terms, "after")}
            <textarea
              ref={ime}
              className="ime-proxy"
              aria-hidden
              tabIndex={-1}
              value=""
              onChange={() => undefined}
            />
          </div>
        </section>
      ) : (
        <section
          className="editor-segment is-context"
          data-entry={entry.entryNumber}
          data-marker-count={entry.marks.length}
          key={entry.key}
          role="button"
          tabIndex={0}
          onClick={() => void select(entry.index)}
          onKeyDown={(ev) => {
            if (ev.key === "Enter" || ev.key === " ") {
              ev.preventDefault();
              void select(entry.index);
            }
          }}
        >
          <div className="segment-meta">{entry.file} · #{entry.entryNumber}</div>
          <div className="src">
            {renderRichText(entry.source, 0, marks, terms, `source-${entry.key}`)}
          </div>
          <div className="tgt">
            {entry.translation
              ? renderRichText(entry.translation, 0, marks, terms, `target-${entry.key}`)
              : <span className="muted">{entry.source}</span>}
          </div>
        </section>
      ))}
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
