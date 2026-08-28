export type RenderedCaretHit = {
  offset: number;
  bias: "before" | "after";
  fragmentStart: number;
  fragmentEnd: number;
};

export type RenderedFragmentMetrics = {
  offset: number;
  sourceLength: number;
  renderedLength: number;
  atomic?: boolean;
};

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

/**
 * Convert a visible UTF-16 offset inside one decorated fragment back to the
 * undecorated model. Expanded marker glyphs and protected/tag fragments are
 * atomic and choose their side by the pointer's visual half.
 */
export function modelOffsetForRenderedPosition(
  metrics: RenderedFragmentMetrics,
  localRenderedOffset: number,
): RenderedCaretHit {
  const renderedLength = Math.max(0, metrics.renderedLength);
  const sourceLength = Math.max(0, metrics.sourceLength);
  const local = Math.max(0, Math.min(localRenderedOffset, renderedLength));
  const bias = local * 2 < renderedLength ? "before" : "after";
  if (metrics.atomic || renderedLength !== sourceLength) {
    return {
      offset: metrics.offset + (bias === "after" ? sourceLength : 0),
      bias,
      fragmentStart: metrics.offset,
      fragmentEnd: metrics.offset + sourceLength,
    };
  }
  return {
    offset: metrics.offset + Math.min(local, sourceLength),
    bias,
    fragmentStart: metrics.offset,
    fragmentEnd: metrics.offset + sourceLength,
  };
}

function caretHitFromPoint(doc: Document, x: number, y: number): NativeCaretHit | null {
  const native = doc as CaretCapableDocument;
  const position = native.caretPositionFromPoint?.(x, y);
  if (position) return { node: position.offsetNode, offset: position.offset };
  const range = native.caretRangeFromPoint?.(x, y);
  if (range) return { node: range.startContainer, offset: range.startOffset };
  return null;
}

function fragmentMetrics(fragment: HTMLElement): RenderedFragmentMetrics | null {
  const offset = Number(fragment.dataset.offset);
  const sourceLength = Number(fragment.dataset.sourceLength);
  if (!Number.isFinite(offset) || !Number.isFinite(sourceLength)) return null;
  return {
    offset,
    sourceLength,
    renderedLength: fragment.textContent?.length ?? 0,
    atomic: fragment.dataset.atomic === "true",
  };
}

function closestFragment(node: Node | null, root: HTMLElement): HTMLElement | null {
  const origin =
    node?.nodeType === 1
      ? node as Element
      : node?.parentElement;
  const fragment = origin?.closest<HTMLElement>("[data-offset][data-source-length]") ?? null;
  return fragment && root.contains(fragment) ? fragment : null;
}

/**
 * Resolve Chromium's caret point through arbitrarily nested marker wrappers.
 * Range text length is UTF-16, matching Java Swing document offsets.
 */
export function renderedCaretFromPoint(
  root: HTMLElement,
  x: number,
  y: number,
): RenderedCaretHit | null {
  const doc = root.ownerDocument;
  const hit = caretHitFromPoint(doc, x, y);
  const fragment = closestFragment(hit?.node ?? null, root);
  if (hit && fragment) {
    const metrics = fragmentMetrics(fragment);
    if (metrics) {
      const limit =
        hit.node.nodeType === 3
          ? hit.node.textContent?.length ?? 0
          : hit.node.childNodes.length;
      const range = doc.createRange();
      range.selectNodeContents(fragment);
      try {
        range.setEnd(hit.node, Math.max(0, Math.min(hit.offset, limit)));
        return modelOffsetForRenderedPosition(metrics, range.toString().length);
      } catch {
        // A React layout pass can replace Chromium's transient caret node.
      }
    }
  }

  const fallback = doc
    .elementFromPoint(x, y)
    ?.closest<HTMLElement>("[data-offset][data-source-length]");
  if (!fallback || !root.contains(fallback)) return null;
  const metrics = fragmentMetrics(fallback);
  if (!metrics) return null;
  const rect = fallback.getBoundingClientRect();
  return modelOffsetForRenderedPosition(
    metrics,
    x < rect.left + rect.width / 2 ? 0 : metrics.renderedLength,
  );
}
