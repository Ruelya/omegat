export type RelativeEditorSelection = {
  anchor: number;
  focus: number;
};

/** Keep a relative caret/selection only while the complete EntryKey is stable. */
export function selectionAfterEntryChange(
  previousEntryKey: string | null,
  currentEntryKey: string | null,
  selection: RelativeEditorSelection,
  translationLength: number,
): RelativeEditorSelection {
  const limit = Math.max(0, translationLength);
  if (previousEntryKey !== currentEntryKey) {
    return { anchor: limit, focus: limit };
  }
  return {
    anchor: Math.max(0, Math.min(selection.anchor, limit)),
    focus: Math.max(0, Math.min(selection.focus, limit)),
  };
}

/**
 * Match Java's deterministic filtered rebuild: prefer the next visible entry,
 * then wrap to the first one.
 */
export function nextUntranslatedEntryIndex(
  entries: readonly { translation: string }[],
  activeIndex: number,
): number {
  const after = entries.findIndex(
    (entry, index) => index > activeIndex && !entry.translation,
  );
  if (after >= 0) return after;
  return entries.findIndex((entry) => !entry.translation);
}
