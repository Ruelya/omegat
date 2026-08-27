import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
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
import {
  EditorController,
  type EditorScrollAnchor,
  type ScrollAnchorCandidate,
} from "./EditorController";
import { bindMarkerRemark } from "./IEditor";
import { tooltipTextsAt } from "./MarkerController";
import { editorPopups } from "./EditorPopups";
import { EditorTextArea3 } from "./EditorTextArea3";
import type { EntryPart, Mark } from "./mark/Mark";
import { NativePluginMarkerBridge } from "./mark/NativePluginMarker";

const editorController = new EditorController();
const nativePluginMarkers = new NativePluginMarkerBridge(editorController);

type NativeCaretHit = {
  node: Node;
  offset: number;
};

type CaretCapableDocument = Document & {
  caretPositionFromPoint?: (
    x: number,
    y: number,
  ) => { offsetNode: Node; offset: number } | null;
  caretRangeFromPoint?: (x: number, y: number) => Range | null;
};

function caretHitFromPoint(doc: Document, x: number, y: number): NativeCaretHit | null {
  const native = doc as CaretCapableDocument;
  const position = native.caretPositionFromPoint?.(x, y);
  if (position) return { node: position.offsetNode, offset: position.offset };
  const range = native.caretRangeFromPoint?.(x, y);
  if (range) return { node: range.startContainer, offset: range.startOffset };
  return null;
}

function renderedCaretFromPoint(
  root: HTMLElement,
  x: number,
  y: number,
): { offset: number; bias: "before" | "after" } | null {
  const doc = root.ownerDocument;
  const hit = caretHitFromPoint(doc, x, y);
  if (hit) {
    const origin =
      hit.node.nodeType === Node.ELEMENT_NODE
        ? hit.node as Element
        : hit.node.parentElement;
    const fragment = origin?.closest<HTMLElement>("[data-offset]");
    if (fragment && root.contains(fragment)) {
      const start = Number(fragment.dataset.offset);
      if (Number.isFinite(start)) {
        const limit =
          hit.node.nodeType === Node.TEXT_NODE
            ? hit.node.textContent?.length ?? 0
            : hit.node.childNodes.length;
        const range = doc.createRange();
        range.selectNodeContents(fragment);
        try {
          range.setEnd(hit.node, Math.max(0, Math.min(hit.offset, limit)));
          const local = range.toString().length;
          const length = fragment.textContent?.length ?? 0;
          const bias =
            fragment.dataset.tag && local * 2 < length ? "before" : "after";
          return { offset: start + local, bias };
        } catch {
          // Fall through to an element-boundary hit when Chromium gives a
          // transient node that was replaced during the same layout pass.
        }
      }
    }
  }

  const fragment = doc.elementFromPoint(x, y)?.closest<HTMLElement>("[data-offset]");
  if (!fragment || !root.contains(fragment)) return null;
  const start = Number(fragment.dataset.offset);
  if (!Number.isFinite(start)) return null;
  const rect = fragment.getBoundingClientRect();
  const after = x >= rect.left + rect.width / 2;
  return {
    offset: start + (after ? fragment.textContent?.length ?? 0 : 0),
    bias: after ? "after" : "before",
  };
}

function scrollCandidates(container: HTMLElement): ScrollAnchorCandidate[] {
  return [...container.querySelectorAll<HTMLElement>("[data-entry-key]")].map((element) => {
    const bounds = element.getBoundingClientRect();
    return {
      key: element.dataset.entryKey ?? "",
      top: bounds.top,
      bottom: bounds.bottom,
    };
  });
}

function renderRichText(
  text: string,
  offset: number,
  marks: ReturnType<typeof useApp.getState>["marks"],
  terms: string[],
  keyPrefix: string,
  productMarks: Mark[] = [],
  entryPart: EntryPart = "TRANSLATION",
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
    const relevant = productMarks.filter((mark) =>
      mark.entryPart === entryPart
      && mark.startOffset < start + tok.value.length
      && mark.endOffset > start
    );
    const boundaries = new Set([0, tok.value.length]);
    for (const mark of relevant) {
      boundaries.add(Math.max(0, mark.startOffset - start));
      boundaries.add(Math.min(tok.value.length, mark.endOffset - start));
    }
    const points = [...boundaries].sort((a, b) => a - b);
    return points.slice(0, -1).flatMap((sliceStart, sliceIndex) => {
      const sliceEnd = points[sliceIndex + 1]!;
      const raw = tok.value.slice(sliceStart, sliceEnd);
      const sliceMarks = relevant.filter((mark) =>
        mark.startOffset < start + sliceEnd
        && mark.endOffset > start + sliceStart
      );
      const markerClasses = sliceMarks
        .map((mark) =>
          mark.painter === "spell"
            ? "mark-spell"
            : `product-marker-${mark.painter.replace(/[^a-z0-9_-]/gi, "-")}`,
        );
      const markerTooltip = tooltipTextsAt(
        sliceMarks,
        entryPart,
        start + sliceStart,
      ).join("\n");
      return decorateText(raw, marks, terms).map((span, decoratedIndex) => (
        <span
          key={`${keyPrefix}-text-${i}-${sliceIndex}-${decoratedIndex}`}
          className={[...span.cls, ...markerClasses].join(" ")}
          data-offset={start + sliceStart}
          title={markerTooltip || undefined}
        >
          {span.text}
        </span>
      ));
    });
  });
}

export function SegmentSource({ productMarks = [] }: { productMarks?: Mark[] }) {
  const e = useApp((s) => s.entries[s.index]);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  const selected = useApp((s) => s.selectedText);
  if (!e || !marks.displaySource) return null;
  const terms = glossary.map((g) => g.source);
  return (
    <div className={`src ${sourceClass(e, marks)} ${selected === e.source ? "is-selected-source" : ""}`}>
      {renderRichText(e.source, 0, marks, terms, "active-source", productMarks, "SOURCE")}
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
  const open = useApp((s) => s.open);
  const importPaths = useApp((s) => s.importPaths);
  const projectLoaded = useApp((s) => Boolean(s.props));
  const completer = useApp((s) => s.completer);
  const queryCompleter = useApp((s) => s.queryCompleter);
  const marks = useApp((s) => s.marks);
  const glossary = useApp((s) => s.glossary);
  const focus = useApp((s) => s.focusPanel);
  const tabAdvance = useApp((s) => Boolean(s.prefs?.tab_advance));
  const editConflict = useApp((s) => s.editConflict);
  const resolveEditConflict = useApp((s) => s.resolveEditConflict);
  const scrollViewport = useRef<HTMLDivElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const ime = useRef<HTMLTextAreaElement>(null);
  const interaction = useRef(new EditorTextArea3());
  const pendingScrollAnchor = useRef<EditorScrollAnchor | null>(null);
  const [selection, setSelection] = useState({
    anchor: document3.translation.length,
    focus: document3.translation.length,
  });
  const [pageRadius, setPageRadius] = useState(8);
  const [manualConflict, setManualConflict] = useState("");
  const [, setMarkerRevision] = useState(0);
  const composing = useRef(false);
  const discardCompositionEnd = useRef(false);
  const dragPointer = useRef<number | null>(null);
  const suppressClick = useRef(false);
  editorController.setPageRadius(pageRadius);
  const loadedPage = editorController.synchronizeRendererProject(entries, activeIndex, document3);
  const activeLoadedEntry = loadedPage.find((entry) => entry.active);
  const loadedPageSignature = loadedPage.map(({ key }) => key).join("\u0000");

  useEffect(() => {
    let current = true;
    const unbind = bindMarkerRemark((name) => {
      editorController.remarkOneMarker(name);
      setMarkerRevision((revision) => revision + 1);
      void editorController.refreshCurrentMarkersAsync().then((applied) => {
        if (current && applied) setMarkerRevision((revision) => revision + 1);
      });
    });
    return () => {
      current = false;
      unbind();
    };
  }, []);

  useEffect(() => {
    let current = true;
    const connection = nativePluginMarkers.connect();
    void connection.ready
      .then((installed) => {
        if (!current || !installed) return;
        return editorController.refreshCurrentMarkersAsync().then((applied) => {
          if (current && applied) setMarkerRevision((revision) => revision + 1);
        });
      })
      .catch(() => undefined);
    return () => {
      current = false;
      connection.release();
    };
  }, []);

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
    if (editConflict) setManualConflict(editConflict.ours);
  }, [editConflict]);

  useEffect(() => {
    let current = true;
    void editorController.refreshCurrentMarkersAsync().then((applied) => {
      if (current && applied) setMarkerRevision((revision) => revision + 1);
    });
    return () => {
      current = false;
    };
  }, [activeIndex, document3.source, document3.translation, loadedPageSignature]);

  useEffect(() => {
    if (focus === "editor") surface.current?.focus();
  }, [focus]);

  useEffect(() => {
    const proxy = ime.current;
    if (!proxy) return;
    const onBeforeInput = (event: Event) =>
      onNativeBeforeInput(event as InputEvent);
    const onCompositionStart = () => {
      discardCompositionEnd.current = false;
      beginComposition();
    };
    const onCompositionUpdate = (event: Event) =>
      updateComposition((event as CompositionEvent).data);
    const onCompositionEnd = (event: Event) =>
      finishComposition((event as CompositionEvent).data);
    proxy.addEventListener("beforeinput", onBeforeInput);
    proxy.addEventListener("compositionstart", onCompositionStart);
    proxy.addEventListener("compositionupdate", onCompositionUpdate);
    proxy.addEventListener("compositionend", onCompositionEnd);
    return () => {
      proxy.removeEventListener("beforeinput", onBeforeInput);
      proxy.removeEventListener("compositionstart", onCompositionStart);
      proxy.removeEventListener("compositionupdate", onCompositionUpdate);
      proxy.removeEventListener("compositionend", onCompositionEnd);
    };
  });

  useLayoutEffect(() => {
    const viewport = scrollViewport.current;
    const anchor = pendingScrollAnchor.current;
    pendingScrollAnchor.current = null;
    if (!viewport || !anchor) return;
    const adjustment = editorController.scrollAdjustmentForAnchor(
      anchor,
      viewport.getBoundingClientRect().top,
      scrollCandidates(viewport),
    );
    if (adjustment !== 0) viewport.scrollTop += adjustment;
  }, [loadedPageSignature]);

  function prepareInteraction(): EditorTextArea3 {
    const area = interaction.current;
    area.setDocument(document3);
    area.setProtectedRanges(
      activeLoadedEntry?.marks.flatMap((mark) =>
        mark.entryPart === "TRANSLATION" && mark.painter === "protected"
          ? [{
              start: mark.startOffset,
              end: mark.endOffset,
              tooltip: mark.toolTipText,
            }]
          : []
      ) ?? [],
    );
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
    const current = interaction.current;
    if (current.isComposing()) {
      composing.current = true;
      return;
    }
    const area = prepareInteraction();
    composing.current = area.beginComposition();
  }

  function updateComposition(data: string) {
    const area = interaction.current;
    if (!area.isComposing() || !area.updateComposition(data)) return;
    const next = area.getOmDocument();
    applyDoc(next, area);
  }

  function finishComposition(data: string) {
    if (discardCompositionEnd.current) {
      discardCompositionEnd.current = false;
      composing.current = false;
      return;
    }
    const area = interaction.current;
    const hadNativeComposition = composing.current;
    if (area.isComposing()) {
      area.commitComposition(data);
      const next = area.getOmDocument();
      applyDoc(next, area);
    } else if (!hadNativeComposition && data) {
      insertAt(data);
    }
    composing.current = false;
  }

  function onNativeBeforeInput(native: InputEvent) {
    const isCompositionInput =
      native.inputType === "insertCompositionText"
      || native.inputType === "deleteCompositionText"
      || native.inputType === "insertFromComposition"
      || (native.inputType === "insertText" && composing.current);
    const area = isCompositionInput ? interaction.current : prepareInteraction();
    if (!area.handleBeforeInput(native.inputType, native.data)) return;
    native.preventDefault();
    applyDoc(area.getOmDocument(), area);
  }

  function onKey(ev: KeyboardEvent<HTMLDivElement>) {
    if (interaction.current.isComposing()) {
      if (ev.key === "Escape" && interaction.current.cancelComposition()) {
        ev.preventDefault();
        discardCompositionEnd.current = true;
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
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      void commit().catch(() => undefined);
      return;
    }
    if (ev.key === "Enter" && ev.shiftKey) {
      ev.preventDefault();
      insertAt("\n");
      return;
    }
    if (ev.key === "Tab" && tabAdvance) {
      ev.preventDefault();
      void commit().catch(() => undefined);
      return;
    }
    if (ev.key === "Tab" && !tabAdvance && completer[0]) {
      ev.preventDefault();
      insertAt(completer[0].text);
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
  }

  function onClick(ev: MouseEvent<HTMLDivElement>) {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    const root = surface.current;
    if (!root) return;
    const hit = renderedCaretFromPoint(root, ev.clientX, ev.clientY);
    if (!hit) return;
    const area = prepareInteraction();
    area.setCaretFromRenderedOffset(hit.offset, hit.bias, ev.shiftKey);
    readSelection(area);
  }

  function onPointerDown(ev: PointerEvent<HTMLDivElement>) {
    if (ev.button !== 0 || !ev.isPrimary) return;
    const root = surface.current;
    if (!root) return;
    const hit = renderedCaretFromPoint(root, ev.clientX, ev.clientY);
    if (!hit) return;
    ev.preventDefault();
    dragPointer.current = ev.pointerId;
    suppressClick.current = true;
    ev.currentTarget.setPointerCapture(ev.pointerId);
    const area = prepareInteraction();
    area.beginMouseSelection(hit.offset, hit.bias, ev.shiftKey);
    area.focus();
    readSelection(area);
    ime.current?.focus({ preventScroll: true });
  }

  function onPointerMove(ev: PointerEvent<HTMLDivElement>) {
    const root = surface.current;
    const hit = root
      ? renderedCaretFromPoint(root, ev.clientX, ev.clientY)
      : null;
    const tooltip = hit && activeLoadedEntry
      ? editorController.markers.getToolTips(
          activeLoadedEntry.key,
          "TRANSLATION",
          hit.offset,
        )
      : null;
    if (tooltip) ev.currentTarget.dataset.markerTooltip = tooltip;
    else delete ev.currentTarget.dataset.markerTooltip;

    if (dragPointer.current !== ev.pointerId || !interaction.current.isMouseSelecting()) return;
    if ((ev.buttons & 1) === 0) {
      finishPointerSelection(ev);
      return;
    }
    if (!root) return;
    if (!hit) return;
    ev.preventDefault();
    if (interaction.current.updateMouseSelection(hit.offset, hit.bias)) {
      readSelection(interaction.current);
    }
  }

  function finishPointerSelection(ev: PointerEvent<HTMLDivElement>) {
    if (dragPointer.current !== ev.pointerId) return;
    ev.preventDefault();
    const root = surface.current;
    const hit = root
      ? renderedCaretFromPoint(root, ev.clientX, ev.clientY)
      : null;
    interaction.current.endMouseSelection(hit?.offset, hit?.bias);
    readSelection(interaction.current);
    dragPointer.current = null;
    if (ev.currentTarget.hasPointerCapture(ev.pointerId)) {
      ev.currentTarget.releasePointerCapture(ev.pointerId);
    }
  }

  function cancelPointerSelection(ev: PointerEvent<HTMLDivElement>) {
    if (dragPointer.current !== ev.pointerId) return;
    interaction.current.endMouseSelection();
    readSelection(interaction.current);
    dragPointer.current = null;
  }

  function onDoubleClick(ev: MouseEvent<HTMLDivElement>) {
    const root = surface.current;
    if (!root) return;
    const hit = renderedCaretFromPoint(root, ev.clientX, ev.clientY);
    if (!hit) return;
    const area = prepareInteraction();
    if (area.selectProtectedPartAt(document3.translationStart + hit.offset)) {
      ev.preventDefault();
      readSelection(area);
    }
  }

  function onFileDragOver(ev: DragEvent<HTMLDivElement>) {
    if (![...ev.dataTransfer.types].includes("Files")) return;
    ev.preventDefault();
    ev.dataTransfer.dropEffect = projectLoaded ? "copy" : "link";
  }

  async function onFileDrop(ev: DragEvent<HTMLDivElement>) {
    if (![...ev.dataTransfer.types].includes("Files")) return;
    ev.preventDefault();
    const paths = [...ev.dataTransfer.files]
      .map((file) => window.omegat.pathForFile?.(file) ?? "")
      .filter(Boolean);
    const drop = window.omegat.inspectDrop
      ? await window.omegat.inspectDrop(paths)
      : { kind: "files" as const, paths };
    const result = await editorController.handleFileDrop(drop, projectLoaded, {
      openProject: open,
      importFiles: importPaths,
    });
    useApp.getState().logLine(
      result.accepted
        ? `editor drop ${result.action}: ${result.paths.join(", ")}`
        : "editor drop rejected",
    );
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
    const area = interaction.current;
    const wasComposing = area.isComposing();
    area.blur();
    if (wasComposing) applyDoc(area.getOmDocument(), area);
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
      pendingScrollAnchor.current = editorController.captureScrollAnchor(
        el.getBoundingClientRect().top,
        scrollCandidates(el),
      );
      setPageRadius((radius) => {
        const next = Math.min(entries.length, radius + 8);
        if (next === radius) pendingScrollAnchor.current = null;
        return next;
      });
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
      ref={scrollViewport}
      className="editor-doc"
      data-first-loaded={editorController.getLoadedRange().first}
      data-last-loaded={editorController.getLoadedRange().last}
      onScroll={onPageScroll}
      onDragOver={onFileDragOver}
      onDrop={(event) => void onFileDrop(event)}
    >
      {editConflict?.index === activeIndex && (
        <div className="hit" role="alert" data-editor-conflict>
          <strong>Translation conflict</strong>
          <div>previous: {editConflict.previous || "—"}</div>
          <div>ours: {editConflict.ours || "—"}</div>
          <div>theirs: {editConflict.theirs || "—"}</div>
          <input
            aria-label="Manual conflict translation"
            value={manualConflict}
            onChange={(event) => setManualConflict(event.target.value)}
          />
          <div className="btn-row">
            <button
              type="button"
              onClick={() => void resolveEditConflict("ours").catch(() => undefined)}
            >
              {t("keepOurs")}
            </button>
            <button
              type="button"
              onClick={() => void resolveEditConflict("theirs").catch(() => undefined)}
            >
              {t("keepTheirs")}
            </button>
            <button
              type="button"
              onClick={() =>
                void resolveEditConflict("manual", manualConflict).catch(() => undefined)}
            >
              Manual
            </button>
          </div>
        </div>
      )}
      {loadedPage.map((entry) => entry.active ? (
        <section
          className="editor-segment is-active"
          data-entry={entry.entryNumber}
          data-entry-key={entry.key}
          key={entry.key}
        >
          <div className="segment-meta">{entry.file} · #{entry.entryNumber}</div>
          <div className="pane-h">{t("source")}</div>
          <SegmentSource productMarks={entry.marks} />
          <div className="pane-h">{t("target")}</div>
          <div
            ref={surface}
            className="tgt editor-surface"
            tabIndex={0}
            role="textbox"
            aria-multiline="true"
            aria-label={t("target")}
            onKeyDown={onKey}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={finishPointerSelection}
            onPointerCancel={cancelPointerSelection}
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
          >
            {renderRichText(beforeSelection, 0, marks, terms, "before", entry.marks)}
            {selected && selection.focus === selectionStart && <span className="caret" aria-hidden />}
            {selected && (
              <span className="editor-selection">
                {renderRichText(selected, selectionStart, marks, terms, "selection", entry.marks)}
              </span>
            )}
            {(!selected || selection.focus === selectionEnd) && <span className="caret" aria-hidden />}
            {renderRichText(afterSelection, selectionEnd, marks, terms, "after", entry.marks)}
            <textarea
              ref={ime}
              className="ime-proxy"
              aria-hidden
              tabIndex={-1}
              value=""
              onChange={() => undefined}
              onInput={(ev) => {
                ev.currentTarget.value = "";
              }}
            />
          </div>
        </section>
      ) : (
        <section
          className="editor-segment is-context"
          data-entry={entry.entryNumber}
          data-entry-key={entry.key}
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
            {renderRichText(
              entry.source,
              0,
              marks,
              terms,
              `source-${entry.key}`,
              entry.marks,
              "SOURCE",
            )}
          </div>
          <div className="tgt">
            {entry.translation
              ? renderRichText(
                  entry.translation,
                  0,
                  marks,
                  terms,
                  `target-${entry.key}`,
                  entry.marks,
                )
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
