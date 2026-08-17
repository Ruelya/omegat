import { useEffect, useRef } from "react";
import { decorateText, parseDocument, serializeFromElement } from "../lib/editor-doc";
import { t } from "../i18n";
import { useApp } from "../store/app";

export function SegmentSource() {
  const e = useApp((s) => s.entries[s.index]);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  if (!e || !marks.displaySource) return null;
  const terms = glossary.map((g) => g.source);
  return (
    <div className={`src ${sourceClass(e, marks)}`}>
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
  const tabAdvance = useApp((s) => s.prefs?.extra.tab_advance === "true");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (document.activeElement === el && serializeFromElement(el) === draft) return;
    el.innerHTML = "";
    const terms = glossary.map((g) => g.source);
    for (const tok of parseDocument(draft)) {
      if (tok.kind === "tag") {
        const span = document.createElement("span");
        span.className = "tag tag-protected";
        span.contentEditable = "false";
        span.dataset.tag = tok.value;
        span.textContent = tok.value;
        el.appendChild(span);
      } else {
        for (const sp of decorateText(tok.value, marks, terms)) {
          const span = document.createElement("span");
          if (sp.cls.length) span.className = sp.cls.join(" ");
          span.textContent = sp.text;
          el.appendChild(span);
        }
      }
    }
  }, [draft, marks, glossary]);

  useEffect(() => {
    if (focus === "editor") ref.current?.focus();
  }, [focus]);

  return (
    <div className="editor-doc">
      <div className="pane-h">{t("target")}</div>
      <div
        ref={ref}
        className="tgt"
        contentEditable
        suppressContentEditableWarning
        role="textbox"
        aria-label={t("target")}
        onInput={() => {
          if (!ref.current) return;
          const text = serializeFromElement(ref.current);
          setDraft(text);
          void queryCompleter(text.split(/\s+/).pop() || "");
        }}
        onKeyDown={(ev) => {
          if (ev.key === "Enter" && !ev.shiftKey) {
            ev.preventDefault();
            void commit();
          }
          if (ev.key === "Tab" && tabAdvance) {
            ev.preventDefault();
            void commit();
          }
          if (ev.key === "Tab" && !tabAdvance && completer[0]) {
            ev.preventDefault();
            setDraft(useApp.getState().draft + completer[0].text);
          }
        }}
      />
      {completer.length > 0 && (
        <div className="completer">
          {completer.slice(0, 8).map((c, i) => (
            <button
              key={`${c.kind}-${c.text}-${i}`}
              type="button"
              className="hit"
              onClick={() => setDraft(useApp.getState().draft + c.text)}
            >
              <span className="score">{c.kind}</span> {c.text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
