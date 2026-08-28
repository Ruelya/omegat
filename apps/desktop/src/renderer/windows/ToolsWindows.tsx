import { useEffect, useMemo, useRef, useState } from "react";
import { t } from "../i18n";
import {
  alignmentDragViewport,
  alignmentRows,
  alignmentPointerSelection,
  alignmentScrollTarget,
  alignmentSelectionAfterEdit,
  alignTableDrop,
  alignTableKey,
  selectionBounds,
  type AlignBead,
  type AlignPinpoint,
  type AlignSide,
  type AlignTableDropResult,
} from "../lib/align-rows";
import {
  repositoriesFromEditorRows,
  repositoryEditorRows,
  type RepositoryEditorRow,
} from "../lib/project-ui";
import type {
  FilterOptionsDto,
  ProjectPropsDto,
  TransactionEnvelope,
} from "../lib/types";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

async function callerManagedProductRpc(
  method: string,
  params: unknown,
): Promise<unknown> {
  const invoke = window.omegat?.rpcWithTransactionReceipt ?? window.omegat?.rpc;
  if (!invoke) throw new Error("sidecar bridge unavailable");
  return invoke(method, params);
}

async function acknowledgeProductReceipt(
  receipt: TransactionEnvelope | null | undefined,
) {
  if (!receipt) return;
  const result = await window.omegat?.acknowledgeTransactionReceipt?.(
    receipt,
    "succeeded",
  );
  if (result && !result.ack.acknowledged) {
    throw new Error(`transaction receipt ${receipt.batch_id} was not acknowledged`);
  }
}

export function AlignWindow() {
  const runLongOperation = useApp((state) => state.runLongOperation);
  const operation = useApp((state) => state.longOperation);
  const cancelOperation = useApp((state) => state.cancelLongOperation);
  const alignOperationActive = Boolean(
    operation
    && (operation.kind === "align" || operation.kind === "alignWrite")
    && (
      operation.phase === "started"
      || operation.phase === "progress"
      || operation.phase === "cancelling"
    ),
  );
  const [src, setSrc] = useState("");
  const [tgt, setTgt] = useState("");
  const [dest, setDest] = useState("");
  const [mode, setMode] = useState("parsewise");
  const [algo, setAlgo] = useState("viterbi");
  const [counter, setCounter] = useState("word");
  const [calculator, setCalculator] = useState("normal");
  const [side, setSide] = useState<AlignSide>("both");
  const [beads, setBeads] = useState<AlignBead[]>([]);
  const [sel, setSel] = useState(0);
  const [anchor, setAnchor] = useState(0);
  const [pinpoint, setPinpoint] = useState<AlignPinpoint | null>(null);
  const [spanText, setSpanText] = useState("");
  const [message, setMessage] = useState("");
  const spanEditor = useRef<HTMLTextAreaElement>(null);
  const tableScroll = useRef<HTMLDivElement>(null);
  const alignTable = useRef<HTMLTableElement>(null);
  const draggedRows = useRef<{
    startRow: number;
    endRow: number;
    side: Exclude<AlignSide, "both">;
  } | null>(null);
  const dragPointer = useRef<{
    clientY: number;
    side: Exclude<AlignSide, "both">;
  } | null>(null);
  const dragScrollFrame = useRef<number | null>(null);
  const [dragTarget, setDragTarget] = useState<{
    row: number;
    side: Exclude<AlignSide, "both">;
  } | null>(null);
  const rows = useMemo(() => alignmentRows(beads), [beads]);
  const selectedRows = selectionBounds(anchor, sel, rows.length);
  function visibleTableRows() {
    const viewport = tableScroll.current;
    if (!viewport || rows.length === 0) return null;
    const viewportRect = viewport.getBoundingClientRect();
    const visible = [...viewport.querySelectorAll<HTMLTableRowElement>("tr[data-align-row]")]
      .filter((row) => {
        const rect = row.getBoundingClientRect();
        return rect.bottom > viewportRect.top && rect.top < viewportRect.bottom;
      })
      .map((row) => Number(row.dataset.alignRow))
      .filter(Number.isInteger);
    if (!visible.length) return null;
    return { firstRow: visible[0], lastRow: visible[visible.length - 1] };
  }
  useEffect(() => {
    if (side === "both") {
      setSpanText("");
      return;
    }
    setSpanText(
      rows
        .slice(selectedRows.start, selectedRows.end + 1)
        .map((row) => (side === "source" ? row.source : row.target))
        .filter((line): line is string => line != null)
        .join("\n"),
    );
  }, [side, rows, selectedRows.start, selectedRows.end]);
  useEffect(() => {
    const viewport = tableScroll.current;
    const visible = visibleTableRows();
    if (!viewport || !visible) return;
    const target = alignmentScrollTarget(visible, anchor, sel, rows.length);
    if (target == null) return;
    const row = viewport.querySelector<HTMLTableRowElement>(
      `tr[data-align-row="${target}"]`,
    );
    if (!row) return;
    const viewportRect = viewport.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    if (rowRect.top < viewportRect.top) {
      viewport.scrollTop += rowRect.top - viewportRect.top;
    } else if (rowRect.bottom > viewportRect.bottom) {
      viewport.scrollTop += rowRect.bottom - viewportRect.bottom;
    }
  }, [anchor, sel, rows]);
  useEffect(
    () => () => {
      if (dragScrollFrame.current != null) {
        cancelAnimationFrame(dragScrollFrame.current);
      }
    },
    [],
  );
  async function run() {
    let r: {
      pairs?: { source: string; target: string }[];
      beads?: AlignBead[];
      receipt?: TransactionEnvelope | null;
    };
    try {
      r = await runLongOperation("align", {
        source: src,
        target: tgt,
        dest,
        mode,
        algo,
        counter,
        calculator,
      });
    } catch (error) {
      setMessage(
        error instanceof Error && error.name === "AbortError"
          ? "alignment cancelled"
          : String(error),
      );
      return;
    }
    const next = Array.isArray(r?.beads)
      ? r.beads
      : (r?.pairs ?? []).map((pair) => ({
          ...pair,
          source_lines: [pair.source],
          target_lines: [pair.target],
          score: Number.MAX_VALUE,
          enabled: true,
          status: "default" as const,
        }));
    setBeads(next);
    setSel(0);
    setAnchor(0);
    setPinpoint(null);
    setMessage("");
    await acknowledgeProductReceipt(r.receipt);
  }
  async function edit(action: string, extra: Record<string, unknown> = {}) {
    const activeRow = rows[sel];
    const beadIndexes = [
      ...new Set(
        rows
          .slice(selectedRows.start, selectedRows.end + 1)
          .map((row) => row.beadIndex),
      ),
    ];
    const rowSpan =
      side === "both"
        ? {}
        : { start_row: selectedRows.start, end_row: selectedRows.end };
    const r = (await window.omegat?.rpc("align.edit", {
      action,
      index: activeRow?.beadIndex ?? 0,
      line_index:
        side === "target"
          ? activeRow?.targetLineIndex ?? 0
          : activeRow?.sourceLineIndex ?? 0,
      indexes: beadIndexes,
      side,
      beads,
      source_lang: useApp.getState().props?.source_lang ?? "en",
      target_lang: useApp.getState().props?.target_lang ?? "fr",
      ...rowSpan,
      ...extra,
    })) as {
      beads?: AlignBead[];
      row_count?: number;
      selection?: { anchor_row: number; focus_row: number } | null;
    };
    if (r?.beads) {
      setBeads(r.beads);
      const rowCount = alignmentRows(r.beads).length;
      const restored = alignmentSelectionAfterEdit(
        { anchor, focus: sel },
        { row_count: r.row_count ?? rowCount, selection: r.selection },
      );
      setAnchor(restored.anchor);
      setSel(restored.focus);
      setMessage("");
    }
  }
  async function write() {
    const props = useApp.getState().props;
    const r = await runLongOperation<{
      count?: number;
      receipt?: TransactionEnvelope | null;
    }>("alignWrite", {
      dest,
      beads,
      source_lang: props?.source_lang ?? "en",
      target_lang: props?.target_lang ?? "fr",
    });
    setMessage(`${r?.count ?? beads.filter((bead) => bead.enabled).length} → ${dest}`);
    await acknowledgeProductReceipt(r.receipt);
  }
  function tableKeyDown(event: React.KeyboardEvent<HTMLTableElement>) {
    const visible = visibleTableRows();
    const result = alignTableKey(
      {
        row: sel,
        anchor,
        rowCount: rows.length,
        side,
        pinpoint,
      },
      {
        key: event.key,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        pageRows: visible ? visible.lastRow - visible.firstRow + 1 : undefined,
      },
    );
    if (!result.handled) return;
    event.preventDefault();
    setSel(result.row);
    setAnchor(result.anchor);
    setSide(result.side);
    setPinpoint(result.pinpoint);
    if (result.focusEditor) spanEditor.current?.focus();
    if (result.action) void edit(result.action, result.extra);
  }
  function startTableDrag(
    event: React.DragEvent<HTMLTableCellElement>,
    row: number,
    dragSide: Exclude<AlignSide, "both">,
  ) {
    if (dragScrollFrame.current != null) {
      cancelAnimationFrame(dragScrollFrame.current);
      dragScrollFrame.current = null;
    }
    dragPointer.current = null;
    setDragTarget(null);
    alignTable.current?.focus({ preventScroll: true });
    const inSelection =
      side === dragSide && row >= selectedRows.start && row <= selectedRows.end;
    draggedRows.current = {
      startRow: inSelection ? selectedRows.start : row,
      endRow: inSelection ? selectedRows.end : row,
      side: dragSide,
    };
    if (!inSelection) {
      setSel(row);
      setAnchor(row);
      setSide(dragSide);
    }
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("application/x-omegat-align-rows", dragSide);
  }
  function tableDropResult(
    row: number,
    targetSide: Exclude<AlignSide, "both">,
  ): AlignTableDropResult {
    const dragged = draggedRows.current;
    return dragged
      ? alignTableDrop(beads, {
          ...dragged,
          targetRow: row,
          targetSide,
        })
      : { allowed: false };
  }
  function focusTableDrop(
    row: number,
    targetSide: Exclude<AlignSide, "both">,
  ) {
    const allowed = tableDropResult(row, targetSide).allowed;
    setDragTarget((current) => {
      const next = allowed ? { row, side: targetSide } : null;
      return current?.row === next?.row && current?.side === next?.side
        ? current
        : next;
    });
    return allowed;
  }
  function dragViewportAt(
    clientY: number,
    targetSide: Exclude<AlignSide, "both">,
  ) {
    const viewport = tableScroll.current;
    const visible = visibleTableRows();
    if (!viewport || !visible) return false;
    const rect = viewport.getBoundingClientRect();
    const result = alignmentDragViewport({
      pointerY: clientY,
      viewportTop: rect.top,
      viewportBottom: rect.bottom,
      scrollTop: viewport.scrollTop,
      scrollHeight: viewport.scrollHeight,
      clientHeight: viewport.clientHeight,
      firstRow: visible.firstRow,
      lastRow: visible.lastRow,
      rowCount: rows.length,
    });
    const allowed =
      result.focusRow == null ? false : focusTableDrop(result.focusRow, targetSide);
    if (result.delta !== 0 && dragScrollFrame.current == null) {
      dragScrollFrame.current = requestAnimationFrame(continueTableDragScroll);
    }
    return allowed;
  }
  function continueTableDragScroll() {
    dragScrollFrame.current = null;
    const pointer = dragPointer.current;
    const viewport = tableScroll.current;
    const visible = visibleTableRows();
    if (!pointer || !viewport || !visible || !draggedRows.current) return;
    const rect = viewport.getBoundingClientRect();
    const result = alignmentDragViewport({
      pointerY: pointer.clientY,
      viewportTop: rect.top,
      viewportBottom: rect.bottom,
      scrollTop: viewport.scrollTop,
      scrollHeight: viewport.scrollHeight,
      clientHeight: viewport.clientHeight,
      firstRow: visible.firstRow,
      lastRow: visible.lastRow,
      rowCount: rows.length,
    });
    if (result.delta !== 0) {
      viewport.scrollTop += result.delta;
    }
    if (result.focusRow != null) {
      focusTableDrop(result.focusRow, pointer.side);
    }
    if (result.delta !== 0) {
      dragScrollFrame.current = requestAnimationFrame(continueTableDragScroll);
    }
  }
  function stopTableDrag() {
    draggedRows.current = null;
    dragPointer.current = null;
    setDragTarget(null);
    if (dragScrollFrame.current != null) {
      cancelAnimationFrame(dragScrollFrame.current);
      dragScrollFrame.current = null;
    }
  }
  function tableDragOver(
    event: React.DragEvent<HTMLTableCellElement>,
    row: number,
    targetSide: Exclude<AlignSide, "both">,
  ) {
    dragPointer.current = { clientY: event.clientY, side: targetSide };
    const directAllowed = focusTableDrop(row, targetSide);
    const edgeAllowed = dragViewportAt(event.clientY, targetSide);
    if (directAllowed || edgeAllowed) {
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
    }
  }
  function dropTableRows(
    event: React.DragEvent<HTMLTableCellElement>,
    row: number,
    targetSide: Exclude<AlignSide, "both">,
  ) {
    const drop = tableDropResult(row, targetSide);
    stopTableDrag();
    if (!drop.allowed || !drop.action || !drop.extra) {
      alignTable.current?.focus({ preventScroll: true });
      return;
    }
    event.preventDefault();
    void edit(drop.action, drop.extra).finally(() => {
      alignTable.current?.focus({ preventScroll: true });
    });
  }
  const activeDescendant =
    dragTarget != null
      ? dragTarget.row < 0
        ? `align-drop-top-${dragTarget.side}`
        : dragTarget.row >= rows.length
          ? `align-drop-bottom-${dragTarget.side}`
          : `align-cell-${dragTarget.row}-${dragTarget.side}`
      : rows.length === 0
        ? undefined
        : side === "both"
          ? `align-row-${sel}`
          : `align-cell-${sel}-${side}`;
  return (
    <Modal id="align" title={t("aligner")} wide>
      <div className="form">
        <input placeholder="source" value={src} onChange={(e) => setSrc(e.target.value)} />
        <input placeholder="target" value={tgt} onChange={(e) => setTgt(e.target.value)} />
        <input placeholder="out.tmx" value={dest} onChange={(e) => setDest(e.target.value)} />
        <select value={mode} onChange={(e) => setMode(e.target.value)}>
          <option value="heapwise">HEAPWISE</option>
          <option value="parsewise">PARSEWISE</option>
          <option value="id">ID</option>
        </select>
        <select value={algo} onChange={(e) => setAlgo(e.target.value)}>
          <option value="viterbi">Viterbi</option>
          <option value="forward-backward">Forward-Backward</option>
        </select>
        <select value={counter} onChange={(e) => setCounter(e.target.value)}>
          <option value="word">WORD</option>
          <option value="char">CHAR</option>
        </select>
        <select value={calculator} onChange={(e) => setCalculator(e.target.value)}>
          <option value="normal">Normal</option>
          <option value="poisson">Poisson</option>
        </select>
        <select
          value={side}
          onChange={(e) => setSide(e.target.value as AlignSide)}
          aria-label="alignment side"
        >
          <option value="both">source + target</option>
          <option value="source">source</option>
          <option value="target">target</option>
        </select>
        <textarea
          ref={spanEditor}
          aria-label="selected alignment lines"
          disabled={side === "both" || rows.length === 0}
          value={spanText}
          onChange={(event) => setSpanText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              alignTable.current?.focus({ preventScroll: true });
            }
          }}
        />
        <div className="btn-row">
          {alignOperationActive ? (
            <button
              type="button"
              className="primary"
              disabled={operation?.phase === "cancelling"}
              onClick={() => void cancelOperation()}
            >
              {operation?.phase === "cancelling" ? "Cancelling…" : t("cancel")}
            </button>
          ) : (
            <button type="button" className="primary" onClick={() => void run()}>{t("create")}</button>
          )}
          <button type="button" onClick={() => void edit("merge")}>{t("alignMerge")}</button>
          <button type="button" onClick={() => void edit("split")}>{t("alignSplit")}</button>
          <button type="button" onClick={() => void edit("up")}>{t("alignUp")}</button>
          <button type="button" onClick={() => void edit("down")}>{t("alignDown")}</button>
          <button
            type="button"
            disabled={side === "both" || rows.length === 0}
            onClick={() => void edit("replace-span", { lines: spanText.split(/\r?\n/) })}
          >
            apply row span
          </button>
          <button type="button" onClick={() => void edit("accepted")}>✓ accepted</button>
          <button type="button" onClick={() => void edit("needs-review")}>! review</button>
          <button type="button" onClick={() => void edit("clear-status")}>clear mark</button>
          <button
            type="button"
            disabled={side === "both"}
            onClick={() => {
              if (side === "both") return;
              if (!pinpoint) {
                setPinpoint({ row: sel, side });
              } else if (pinpoint.row !== sel && pinpoint.side !== side) {
                void edit("pinpoint", {
                  start_row: pinpoint.row,
                  end_row: sel,
                  side: pinpoint.side,
                  end_side: side,
                }).then(() => setPinpoint(null));
              }
            }}
          >
            {pinpoint ? "pinpoint end" : "pinpoint start"}
          </button>
          <button type="button" onClick={() => void edit("realign-pending", { algo })}>realign pending</button>
          <button type="button" disabled={!dest || !beads.length} onClick={() => void write()}>{t("save")}</button>
        </div>
        {message && <div className="meta">{message}</div>}
        <div className="align-table-scroll" ref={tableScroll}>
          <table
            ref={alignTable}
            className="align-table"
            aria-label="manual alignment table"
            aria-activedescendant={activeDescendant}
            tabIndex={0}
            onKeyDown={tableKeyDown}
          >
          <thead>
            <tr><th>#</th><th>source</th><th>target</th></tr>
          </thead>
          <tbody>
            <tr className="align-drop-edge" aria-label="alignment top boundary">
              <td />
              <td
                id="align-drop-top-source"
                className={
                  dragTarget?.row === -1 && dragTarget.side === "source"
                    ? "drag-target"
                    : undefined
                }
                aria-label="move source before first alignment"
                onDragOver={(event) => tableDragOver(event, -1, "source")}
                onDrop={(event) => dropTableRows(event, -1, "source")}
              />
              <td
                id="align-drop-top-target"
                className={
                  dragTarget?.row === -1 && dragTarget.side === "target"
                    ? "drag-target"
                    : undefined
                }
                aria-label="move target before first alignment"
                onDragOver={(event) => tableDragOver(event, -1, "target")}
                onDrop={(event) => dropTableRows(event, -1, "target")}
              />
            </tr>
            {rows.map((row) => {
              const bead = beads[row.beadIndex];
              const selected =
                row.rowIndex >= selectedRows.start && row.rowIndex <= selectedRows.end;
              return (
                <tr
                  id={`align-row-${row.rowIndex}`}
                  key={`${row.beadIndex}-${row.rowInBead}`}
                  data-align-row={row.rowIndex}
                  className={`${selected ? "sel " : ""}${bead.status}`}
                  onClick={(event) => {
                    const next = alignmentPointerSelection(
                      { anchor, focus: sel },
                      row.rowIndex,
                      rows.length,
                      event.shiftKey,
                    );
                    setSel(next.focus);
                    setAnchor(next.anchor);
                    alignTable.current?.focus({ preventScroll: true });
                  }}
                >
                  <td>
                    {row.rowInBead === 0 && (
                      <>
                        <input
                          aria-label={`keep alignment ${row.beadIndex + 1}`}
                          type="checkbox"
                          checked={bead.enabled}
                          onChange={(event) =>
                            void edit("keep", {
                              indexes: [row.beadIndex],
                              enabled: event.target.checked,
                            })
                          }
                        />
                        {row.beadIndex + 1}
                      </>
                    )}
                  </td>
                  <td
                    id={`align-cell-${row.rowIndex}-source`}
                    className={
                      dragTarget?.row === row.rowIndex && dragTarget.side === "source"
                        ? "drag-target"
                        : undefined
                    }
                    draggable={row.source != null}
                    onClick={() => {
                      setSide("source");
                      alignTable.current?.focus({ preventScroll: true });
                    }}
                    onDragStart={(event) =>
                      startTableDrag(event, row.rowIndex, "source")
                    }
                    onDragEnd={stopTableDrag}
                    onDragOver={(event) =>
                      tableDragOver(event, row.rowIndex, "source")
                    }
                    onDrop={(event) =>
                      dropTableRows(event, row.rowIndex, "source")
                    }
                  >
                    {row.source ?? ""}
                  </td>
                  <td
                    id={`align-cell-${row.rowIndex}-target`}
                    className={
                      dragTarget?.row === row.rowIndex && dragTarget.side === "target"
                        ? "drag-target"
                        : undefined
                    }
                    draggable={row.target != null}
                    onClick={() => {
                      setSide("target");
                      alignTable.current?.focus({ preventScroll: true });
                    }}
                    onDragStart={(event) =>
                      startTableDrag(event, row.rowIndex, "target")
                    }
                    onDragEnd={stopTableDrag}
                    onDragOver={(event) =>
                      tableDragOver(event, row.rowIndex, "target")
                    }
                    onDrop={(event) =>
                      dropTableRows(event, row.rowIndex, "target")
                    }
                  >
                    {row.target ?? ""}
                  </td>
                </tr>
              );
            })}
            <tr className="align-drop-edge" aria-label="alignment bottom boundary">
              <td />
              <td
                id="align-drop-bottom-source"
                className={
                  dragTarget?.row === rows.length && dragTarget.side === "source"
                    ? "drag-target"
                    : undefined
                }
                aria-label="move source after last alignment"
                onDragOver={(event) => tableDragOver(event, rows.length, "source")}
                onDrop={(event) => dropTableRows(event, rows.length, "source")}
              />
              <td
                id="align-drop-bottom-target"
                className={
                  dragTarget?.row === rows.length && dragTarget.side === "target"
                    ? "drag-target"
                    : undefined
                }
                aria-label="move target after last alignment"
                onDragOver={(event) => tableDragOver(event, rows.length, "target")}
                onDrop={(event) => dropTableRows(event, rows.length, "target")}
              />
            </tr>
          </tbody>
          </table>
        </div>
      </div>
    </Modal>
  );
}

export function TeamWindow() {
  const msg = useApp((s) => s.teamMessage);
  const conflicts = useApp((s) => s.teamConflicts);
  const sync = useApp((s) => s.teamSync);
  const commit = useApp((s) => s.teamCommit);
  const resolve = useApp((s) => s.resolveConflict);
  const operation = useApp((s) => s.longOperation);
  const cancelOperation = useApp((s) => s.cancelLongOperation);
  const operationActive = Boolean(
    operation
    && (
      operation.phase === "started"
      || operation.phase === "progress"
      || operation.phase === "cancelling"
    ),
  );
  const teamOperationActive = Boolean(
    operationActive
    && operation
    && (
      operation.kind === "teamSync"
      || operation.kind === "teamCommit"
      || operation.kind === "teamResolve"
    ),
  );
  const [manual, setManual] = useState("");
  return (
    <Modal id="team" title={t("team")}>
      <p data-team-message>{msg || "Git / SVN / HTTP / file · prepare → rebase → commit"}</p>
      {conflicts.map((c, i) => (
        <div
          key={`${c.kind ?? "tmx"}-${c.source ?? i}-${JSON.stringify(c.entry_key ?? null)}`}
          className="hit"
          data-team-conflict-key={JSON.stringify(c.entry_key ?? null)}
        >
          <div>
            <strong>{c.source}</strong>
            {c.kind ? ` · ${c.kind}` : ""}
          </div>
          <div>ours: {c.ours}</div>
          <div>theirs: {c.theirs}</div>
          <p>{c.message}</p>
          <input
            placeholder="manual"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
          />
          <div className="btn-row">
            <button
              type="button"
              data-operation-action="team-resolve-ours"
              disabled={operationActive}
              onClick={() => void resolve("ours", c.source, undefined, c.entry_key)}
            >
              {t("keepOurs")}
            </button>
            <button
              type="button"
              data-operation-action="team-resolve-theirs"
              disabled={operationActive}
              onClick={() => void resolve("theirs", c.source, undefined, c.entry_key)}
            >
              {t("keepTheirs")}
            </button>
            <button
              type="button"
              data-operation-action="team-resolve-manual"
              disabled={operationActive}
              onClick={() => void resolve("manual", c.source, manual, c.entry_key)}
            >
              手工
            </button>
          </div>
        </div>
      ))}
      <div className="btn-row">
        {teamOperationActive ? (
          <button
            type="button"
            className="primary"
            disabled={operation?.phase === "cancelling"}
            onClick={() => void cancelOperation()}
          >
            {operation?.phase === "cancelling" ? "Cancelling…" : t("cancel")}
          </button>
        ) : (
          <>
            <button
              type="button"
              className="primary"
              data-operation-action="team-sync"
              disabled={operationActive}
              onClick={() => void sync()}
            >
              {t("sync")}
            </button>
            <button
              type="button"
              data-operation-action="team-commit-source"
              disabled={operationActive}
              onClick={() => void commit("source")}
            >
              {t("commitSource")}
            </button>
            <button
              type="button"
              data-operation-action="team-commit-target"
              disabled={operationActive}
              onClick={() => void commit("target")}
            >
              {t("commitTarget")}
            </button>
          </>
        )}
        <button
          type="button"
          data-action="open-repository-mapping"
          onClick={() => useApp.getState().openWindow("mapping")}
        >
          {t("accessRoot")}
        </button>
        <button type="button" onClick={() => useApp.getState().openWindow("team", false)}>{t("cancel")}</button>
      </div>
    </Modal>
  );
}

export function MappingWindow() {
  const props = useApp((s) => s.props);
  const [rows, setRows] = useState<RepositoryEditorRow[]>(() =>
    repositoryEditorRows(props?.repositories ?? [], props?.root ?? ""),
  );
  useEffect(() => {
    setRows(repositoryEditorRows(props?.repositories ?? [], props?.root ?? ""));
  }, [props]);
  function update(i: number, patch: Partial<RepositoryEditorRow>) {
    setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  }
  return (
    <Modal id="mapping" title={t("team")} wide>
      <div className="form">
        <p>RepositoriesMappingController</p>
        <table className="stats">
          <thead>
            <tr>
              <th>type</th>
              <th>url</th>
              <th>branch</th>
              <th>local</th>
              <th>repository</th>
              <th>include</th>
              <th>exclude</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i} data-repository-row={i}>
                <td>
                  <select data-setting="repo_type" value={r.repo_type} onChange={(e) => update(i, { repo_type: e.target.value })}>
                    <option value="git">git</option>
                    <option value="svn">svn</option>
                    <option value="http">http</option>
                    <option value="file">file</option>
                  </select>
                </td>
                <td><input data-setting="url" value={r.url} onChange={(e) => update(i, { url: e.target.value })} /></td>
                <td><input data-setting="branch" value={r.branch} onChange={(e) => update(i, { branch: e.target.value })} /></td>
                <td><input data-setting="local" value={r.local} onChange={(e) => update(i, { local: e.target.value })} /></td>
                <td><input data-setting="repository" value={r.repository} onChange={(e) => update(i, { repository: e.target.value })} /></td>
                <td><input data-setting="includes" value={r.includes} onChange={(e) => update(i, { includes: e.target.value })} /></td>
                <td><input data-setting="excludes" value={r.excludes} onChange={(e) => update(i, { excludes: e.target.value })} /></td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="btn-row">
          <button
            type="button"
            onClick={() =>
              setRows([
                ...rows,
                { repo_type: "git", url: "", branch: "", local: "/", repository: "/", includes: "/**", excludes: "" },
              ])
            }
          >
            +
          </button>
          <button
            type="button"
            onClick={async () => {
              const dir = await window.omegat?.pickDir();
              if (dir && rows[0]) update(0, { url: dir });
            }}
          >
            {t("accessRoot")}
          </button>
          <button
            type="button"
            className="primary"
            data-action="save-repositories"
            onClick={async () => {
              const repositories = repositoriesFromEditorRows(rows);
              const result = (await callerManagedProductRpc("team.mapping", {
                repositories,
              })) as {
                props?: ProjectPropsDto;
                receipt?: TransactionEnvelope | null;
              };
              if (result.props) {
                useApp.setState({ props: result.props });
              }
              await acknowledgeProductReceipt(result.receipt);
              useApp.getState().openWindow("mapping", false);
            }}
          >
            {t("save")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

export function FiltersWindow() {
  const app = useApp();
  const [opts, setOpts] = useState<FilterOptionsDto | null>(null);
  useEffect(() => {
    void app.loadFilters();
  }, [app]);
  return (
    <Modal id="filters" title={t("filters")} wide>
      {app.filters.map((f) => (
        <div key={f.id} className="hit">
          <button
            type="button"
            data-filter-open={f.id}
            onClick={async () => {
              const o = (await window.omegat?.rpc("filters.options", { id: f.id })) as FilterOptionsDto;
              setOpts({
                ...o,
                options: {
                  ...o.options,
                  ...(useApp.getState().prefs?.filter_options[o.id] ?? {}),
                },
              });
            }}
          >
            {f.name}
          </button>
          <span className="meta">{f.masks.join(", ")}</span>
        </div>
      ))}
      {opts && (
        <div className="form">
          <h3>{opts.name}</h3>
          {Object.entries(opts.options).map(([k, v]) => (
            <label key={k}>
              {k}
              <input
                data-filter-id={opts.id}
                data-filter-option={k}
                value={v}
                onChange={(e) => {
                  setOpts({
                    ...opts,
                    options: { ...opts.options, [k]: e.target.value },
                  });
                }}
              />
            </label>
          ))}
          <button
            type="button"
            className="primary"
            data-action="save-filter-options"
            onClick={async () => {
              const prefs = useApp.getState().prefs;
              if (!prefs) return;
              await app.patchPrefs({
                filter_options: {
                  ...prefs.filter_options,
                  [opts.id]: opts.options,
                },
              });
              if (useApp.getState().props?.root) {
                await useApp.getState().reloadProject();
              }
              app.openWindow("filters", false);
            }}
          >
            {t("save")}
          </button>
        </div>
      )}
      <button type="button" onClick={() => app.openWindow("filters", false)}>{t("cancel")}</button>
    </Modal>
  );
}

type SrxRule = { lang: string; brk: boolean; before: string; after: string };

function xmlEscape(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;");
}

function parseSrxRules(xml: string): SrxRule[] {
  const rules: SrxRule[] = [];
  const langRe = /<languagerule\s+languagerulename="([^"]+)"[^>]*>([\s\S]*?)<\/languagerule>/gi;
  for (const lm of xml.matchAll(langRe)) {
    const lang = lm[1] ?? "";
    const body = lm[2] ?? "";
    for (const rm of body.matchAll(/<rule([^>]*)>([\s\S]*?)<\/rule>/gi)) {
      const brk = !/break\s*=\s*"no"/i.test(rm[1] ?? "");
      const before = /<beforebreak>([\s\S]*?)<\/beforebreak>/i.exec(rm[2] ?? "")?.[1] ?? "";
      const after = /<afterbreak>([\s\S]*?)<\/afterbreak>/i.exec(rm[2] ?? "")?.[1] ?? "";
      rules.push({ lang, brk, before, after });
    }
  }
  return rules;
}

function rulesToSrx(rules: SrxRule[]): string {
  const byLang = new Map<string, SrxRule[]>();
  for (const r of rules) {
    const list = byLang.get(r.lang) ?? [];
    list.push(r);
    byLang.set(r.lang, list);
  }
  const languageRules = [...byLang.entries()]
    .map(
      ([lang, rs]) =>
        `<languagerule languagerulename="${xmlEscape(lang)}">` +
        rs
          .map(
            (r) =>
              `<rule break="${r.brk ? "yes" : "no"}"><beforebreak>${xmlEscape(r.before)}</beforebreak><afterbreak>${xmlEscape(r.after)}</afterbreak></rule>`,
          )
          .join("") +
        `</languagerule>`,
    )
    .join("");
  const mapRules = [...byLang.keys()]
    .map(
      (lang) =>
        `<maprule><languagemap languagepattern="${xmlEscape(lang)}" languagerulename="${xmlEscape(lang)}"/></maprule>`,
    )
    .join("");
  return `<?xml version="1.0"?><srx><body><languagerules>${languageRules}</languagerules><maprules>${mapRules}</maprules></body></srx>`;
}

export function SegmentationWindow() {
  const prefs = useApp((s) => s.prefs);
  const patch = useApp((s) => s.patchPrefs);
  const [path, setPath] = useState(prefs?.srx_path || "fixtures/srx/defaultRules.srx");
  const [rules, setRules] = useState<SrxRule[]>(() => parseSrxRules(prefs?.srx_xml || ""));
  useEffect(() => {
    setPath(prefs?.srx_path || "fixtures/srx/defaultRules.srx");
    setRules(parseSrxRules(prefs?.srx_xml || ""));
  }, [prefs]);
  return (
    <Modal id="segmentation" title={t("segmentation")} wide>
      <div className="form">
        <label>
          SRX path
          <input data-setting="srx_path" value={path} onChange={(e) => setPath(e.target.value)} />
        </label>
        <table className="stats">
          <thead>
            <tr><th>lang</th><th>break</th><th>before</th><th>after</th></tr>
          </thead>
          <tbody>
            {rules.map((r, i) => (
              <tr key={i}>
                <td><input data-setting="srx_lang" value={r.lang} onChange={(e) => setRules(rules.map((x, j) => j === i ? { ...x, lang: e.target.value } : x))} /></td>
                <td><input type="checkbox" checked={r.brk} onChange={(e) => setRules(rules.map((x, j) => j === i ? { ...x, brk: e.target.checked } : x))} /></td>
                <td><input value={r.before} onChange={(e) => setRules(rules.map((x, j) => j === i ? { ...x, before: e.target.value } : x))} /></td>
                <td><input value={r.after} onChange={(e) => setRules(rules.map((x, j) => j === i ? { ...x, after: e.target.value } : x))} /></td>
              </tr>
            ))}
          </tbody>
        </table>
        <button type="button" onClick={() => setRules([...rules, { lang: "en.*", brk: true, before: "\\.", after: "\\s" }])}>+</button>
        <button
          type="button"
          className="primary"
          data-action="save-segmentation"
          onClick={async () => {
            await patch({ srx_path: path, srx_xml: rulesToSrx(rules) });
            if (useApp.getState().props?.root) {
              await useApp.getState().reloadProject();
            }
            useApp.getState().openWindow("segmentation", false);
          }}
        >
          {t("save")}
        </button>
      </div>
    </Modal>
  );
}

function projectDraft(props: ProjectPropsDto | null) {
  return {
    source_lang: props?.source_lang || "en",
    target_lang: props?.target_lang || "fr",
    source_tok: props?.source_tok || "org.omegat.tokenizer.DefaultTokenizer",
    target_tok: props?.target_tok || "org.omegat.tokenizer.DefaultTokenizer",
    sentence_seg: props?.sentence_seg ?? true,
    source_dir: props?.source_dir || "",
    target_dir: props?.target_dir || "",
    tm_dir: props?.tm_dir || "",
    glossary_dir: props?.glossary_dir || "",
    glossary_file: props?.glossary_file || "",
    dictionary_dir: props?.dictionary_dir || "",
    export_tm_dir: props?.export_tm_dir || "",
    export_tm_levels: props?.export_tm_levels || "",
    support_default_translations: props?.support_default_translations ?? true,
    remove_tags: props?.remove_tags ?? false,
    external_command: props?.external_command || "",
    source_dir_excludes: (props?.source_dir_excludes ?? []).join("\n"),
  };
}

export function ProjectEditWindow() {
  const props = useApp((s) => s.props);
  const [draft, setDraft] = useState(() => projectDraft(props));
  useEffect(() => {
    setDraft(projectDraft(props));
  }, [props]);
  const setText = <K extends keyof typeof draft,>(
    key: K,
    value: (typeof draft)[K],
  ) =>
    setDraft({ ...draft, [key]: value });
  return (
    <Modal id="project-edit" title={t("properties")} wide>
      <div className="form">
        <label>{t("sourceLang")}<input data-setting="source_lang" value={draft.source_lang} onChange={(e) => setText("source_lang", e.target.value)} /></label>
        <label>{t("targetLang")}<input data-setting="target_lang" value={draft.target_lang} onChange={(e) => setText("target_lang", e.target.value)} /></label>
        <label>Source tokenizer<input value={draft.source_tok} onChange={(e) => setText("source_tok", e.target.value)} /></label>
        <label>Target tokenizer<input value={draft.target_tok} onChange={(e) => setText("target_tok", e.target.value)} /></label>
        <label>Source folder<input value={draft.source_dir} onChange={(e) => setText("source_dir", e.target.value)} /></label>
        <label>Target folder<input value={draft.target_dir} onChange={(e) => setText("target_dir", e.target.value)} /></label>
        <label>TM folder<input value={draft.tm_dir} onChange={(e) => setText("tm_dir", e.target.value)} /></label>
        <label>Glossary folder<input value={draft.glossary_dir} onChange={(e) => setText("glossary_dir", e.target.value)} /></label>
        <label>Writable glossary<input value={draft.glossary_file} onChange={(e) => setText("glossary_file", e.target.value)} /></label>
        <label>Dictionary folder<input value={draft.dictionary_dir} onChange={(e) => setText("dictionary_dir", e.target.value)} /></label>
        <label>Export TM folder<input value={draft.export_tm_dir} onChange={(e) => setText("export_tm_dir", e.target.value)} /></label>
        <label>Export TM levels<input value={draft.export_tm_levels} onChange={(e) => setText("export_tm_levels", e.target.value)} /></label>
        <label>External command<input value={draft.external_command} onChange={(e) => setText("external_command", e.target.value)} /></label>
        <label>Source excludes<textarea rows={3} value={draft.source_dir_excludes} onChange={(e) => setText("source_dir_excludes", e.target.value)} /></label>
        <label><input type="checkbox" checked={draft.sentence_seg} onChange={(e) => setDraft({ ...draft, sentence_seg: e.target.checked })} /> {t("sentenceSeg")}</label>
        <label><input type="checkbox" checked={draft.support_default_translations} onChange={(e) => setDraft({ ...draft, support_default_translations: e.target.checked })} /> Default translations</label>
        <label><input type="checkbox" checked={draft.remove_tags} onChange={(e) => setDraft({ ...draft, remove_tags: e.target.checked })} /> Remove tags</label>
        <div className="btn-row">
          <button
            type="button"
            className="primary"
            data-action="save-project-properties"
            onClick={async () => {
              if (props?.root) {
                const result = (await callerManagedProductRpc("project.update", {
                  root: props.root,
                  ...draft,
                  source_dir_excludes: draft.source_dir_excludes
                    .split(/\r?\n/)
                    .map((value) => value.trim())
                    .filter(Boolean),
                })) as {
                  props?: ProjectPropsDto;
                  receipt?: TransactionEnvelope | null;
                };
                if (result.props) {
                  useApp.setState({ props: result.props });
                  await useApp.getState().refreshEntriesAfterExternalChange();
                }
                await acknowledgeProductReceipt(result.receipt);
              }
              useApp.getState().openWindow("project-edit", false);
            }}
          >
            {t("save")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

export function FinderWindow() {
  const prefs = useApp((s) => s.prefs);
  const patch = useApp((s) => s.patchPrefs);
  const [xml, setXml] = useState(prefs?.finder_xml || "<items><item><name>Wiktionary</name><url>https://en.wiktionary.org/wiki/{selection}</url><scope>selection</scope></item></items>");
  const [urls, setUrls] = useState<string[]>([]);
  return (
    <Modal id="finder" title={t("finder")}>
      <div className="form">
        <textarea rows={8} value={xml} onChange={(e) => setXml(e.target.value)} />
        <div className="btn-row">
          <button type="button" className="primary" onClick={() => void patch({ finder_xml: xml })}>{t("save")}</button>
          <button
            type="button"
            onClick={async () => {
              const st = useApp.getState();
              const e = st.entries[st.index];
              const translation = st.document3.translation;
              const sel = st.selectedText || translation || e?.source || "";
              const r = (await window.omegat?.rpc("finder.run", {
                xml,
                selection: sel,
                source: e?.source ?? sel,
                target: translation || e?.translation || "",
              })) as { urls?: string[]; commands?: string[] };
              const next = r?.urls ?? [];
              setUrls(next);
              for (const u of next) {
                await window.omegat?.openExternal(u);
              }
            }}
          >
            {t("run")}
          </button>
        </div>
        {urls.map((u) => (
          <div key={u} className="meta">{u}</div>
        ))}
      </div>
    </Modal>
  );
}

const SHORTCUTS: [string, string][] = [
  ["project.save", "CmdOrCtrl+S"],
  ["project.compile", "CmdOrCtrl+D"],
  ["edit.insert-translation", "CmdOrCtrl+I"],
  ["edit.overwrite-translation", "CmdOrCtrl+R"],
  ["goto.untranslated", "CmdOrCtrl+U"],
  ["goto.next", "CmdOrCtrl+N"],
  ["edit.search", "CmdOrCtrl+F"],
  ["edit.replace", "CmdOrCtrl+K"],
];

export function ShortcutsWindow() {
  const prefs = useApp((s) => s.prefs);
  const patch = useApp((s) => s.patchPrefs);
  return (
    <Modal id="shortcuts" title={t("shortcuts")} wide>
      <table className="stats">
        <tbody>
          {SHORTCUTS.map(([id, def]) => (
            <tr key={id}>
              <td>{id}</td>
              <td>
                <input
                  defaultValue={prefs?.shortcuts[id] || def}
                  onBlur={(e) => {
                    const cur = useApp.getState().prefs;
                    if (!cur) return;
                    void patch({ shortcuts: { ...cur.shortcuts, [id]: e.target.value } });
                  }}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <button type="button" onClick={() => useApp.getState().openWindow("shortcuts", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function WikiWindow() {
  const [src, setSrc] = useState("");
  return (
    <Modal id="wiki" title={t("wiki")}>
      <input
        data-setting="wiki-source"
        value={src}
        onChange={(e) => setSrc(e.target.value)}
        placeholder="page.xml"
      />
      <button
        type="button"
        className="primary"
        data-action="import-wiki"
        onClick={() => void useApp.getState().importWiki(src).then(() => useApp.getState().openWindow("wiki", false))}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function MedWindow() {
  const [src, setSrc] = useState("");
  const [dest, setDest] = useState("");
  return (
    <Modal id="med" title={t("med")}>
      <input value={src} onChange={(e) => setSrc(e.target.value)} placeholder="pack.zip" />
      <input value={dest} onChange={(e) => setDest(e.target.value)} placeholder="dest" />
      <button
        type="button"
        className="primary"
        onClick={async () => {
          await window.omegat?.rpc("med.open", { source: src, dest });
          useApp.getState().openWindow("med", false);
        }}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function ConvertWindow() {
  const [src, setSrc] = useState("");
  const [dest, setDest] = useState("");
  return (
    <Modal id="convert" title={t("convert")}>
      <input value={src} onChange={(e) => setSrc(e.target.value)} />
      <input value={dest} onChange={(e) => setDest(e.target.value)} />
      <button
        type="button"
        className="primary"
        onClick={async () => {
          await window.omegat?.rpc("project.convert", { source: src, dest, source_lang: "en", target_lang: "fr" });
          useApp.getState().openWindow("convert", false);
        }}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function ScriptsWindow() {
  const [src, setSrc] = useState("console.println(editor.getTranslation())");
  const [out, setOut] = useState("");
  return (
    <Modal id="scripts" title={t("scripts")} wide>
      <textarea
        data-setting="script-source"
        rows={8}
        value={src}
        onChange={(e) => setSrc(e.target.value)}
      />
      <button
        type="button"
        className="primary"
        data-action="run-script"
        onClick={async () => {
          const r = (await callerManagedProductRpc("script.run", {
            source: src,
            index: useApp.getState().index,
          })) as {
            result?: string;
            receipt?: TransactionEnvelope | null;
          };
          setOut(String(r?.result ?? ""));
          await useApp.getState().refreshEntriesAfterExternalChange();
          await acknowledgeProductReceipt(r.receipt);
        }}
      >
        {t("run")}
      </button>
      <pre className="log">{out}</pre>
    </Modal>
  );
}

export function TmxExportWindow() {
  const [dest, setDest] = useState("");
  const [level, setLevel] = useState<"omegat" | "level1" | "level2">("level2");
  const [error, setError] = useState("");
  return (
    <Modal id="tmx-export" title={t("exportTm")}>
      <label>
        {t("target")}
        <input
          data-setting="tmx-destination"
          value={dest}
          onChange={(event) => setDest(event.target.value)}
          placeholder="export.tmx"
        />
      </label>
      <label>
        TMX
        <select
          data-setting="tmx-level"
          value={level}
          onChange={(event) =>
            setLevel(event.target.value as "omegat" | "level1" | "level2")}
        >
          <option value="omegat">OmegaT</option>
          <option value="level1">level1</option>
          <option value="level2">level2</option>
        </select>
      </label>
      <button
        type="button"
        className="primary"
        data-action="export-tmx"
        onClick={async () => {
          setError("");
          try {
            await useApp.getState().exportTmx(dest, level);
            useApp.getState().openWindow("tmx-export", false);
          } catch (failure) {
            setError(String(failure));
          }
        }}
      >
        {t("exportTm")}
      </button>
      {error && <div role="alert" data-persistence-error="tmx-export">{error}</div>}
    </Modal>
  );
}

export function GlossaryAddWindow() {
  const [s, setS] = useState("");
  const [tg, setTg] = useState("");
  return (
    <Modal id="glossary-add" title={t("glossary")}>
      <input
        data-setting="glossary-source"
        value={s}
        onChange={(e) => setS(e.target.value)}
        placeholder="source"
      />
      <input
        data-setting="glossary-target"
        value={tg}
        onChange={(e) => setTg(e.target.value)}
        placeholder="target"
      />
      <button
        type="button"
        className="primary"
        data-action="add-glossary"
        onClick={() => void useApp.getState().addGlossary(s, tg).then(() => useApp.getState().openWindow("glossary-add", false))}
      >
        {t("create")}
      </button>
    </Modal>
  );
}
