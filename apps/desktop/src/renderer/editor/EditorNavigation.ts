// SPDX-License-Identifier: GPL-3.0-or-later

import type { EntryKeyDto } from "../lib/types";

export type EditorNavigationEntry = {
  key?: EntryKeyDto;
  file: string;
  source: string;
  translation: string;
  translated?: boolean;
  isAlt?: boolean;
};

/**
 * Java `EntryKey` identity. File/id/source alone are insufficient because
 * duplicate alternatives are separated by their previous/next/path context.
 */
export function sameCompleteEntryKey(
  left: EntryKeyDto | undefined,
  right: EntryKeyDto | undefined,
): boolean {
  return Boolean(
    left
    && right
    && left.file === right.file
    && left.source_text === right.source_text
    && left.id === right.id
    && left.prev === right.prev
    && left.next === right.next
    && left.path === right.path
  );
}

/**
 * Find the next matching entry with Java's project-wide wraparound behavior.
 * The current entry is considered last, after every other entry was checked.
 */
export function findCyclicEntryIndex<T>(
  entries: readonly T[],
  currentIndex: number,
  direction: -1 | 1,
  allowed: (entry: T, index: number) => boolean = () => true,
  matches: (entry: T, index: number) => boolean = () => true,
): number | null {
  if (entries.length === 0) return null;
  const origin = Math.max(0, Math.min(currentIndex, entries.length - 1));
  for (let distance = 1; distance <= entries.length; distance += 1) {
    const index = (origin + direction * distance + entries.length * 2) % entries.length;
    const entry = entries[index]!;
    if (allowed(entry, index) && matches(entry, index)) return index;
  }
  return null;
}

/** Resolve Java's source/complete-key overload without crossing alternatives. */
export function findEntryBySourceAndKey(
  entries: readonly EditorNavigationEntry[],
  source: string | null,
  key: EntryKeyDto | null,
): number {
  return entries.findIndex((entry) => {
    if (source !== null && entry.source !== source) return false;
    if (key !== null) return sameCompleteEntryKey(entry.key, key);
    const translated = entry.translated ?? entry.translation.length > 0;
    return !entry.isAlt && translated;
  });
}

/** Locate the first visible entry in a file, preserving project file order. */
export function findEntryInFile(
  entries: readonly Pick<EditorNavigationEntry, "file">[],
  file: string,
  visible: ReadonlySet<number>,
): number | null {
  const index = entries.findIndex((entry, candidate) =>
    entry.file === file && visible.has(candidate)
  );
  return index < 0 ? null : index;
}

export type ReloadEntryBinding = {
  index: number;
  exact: boolean;
};

/**
 * Rebind an editor lifecycle across reload. The caller supplies complete-key
 * identity; a missing key falls back deterministically to the old position.
 */
export function rebindEntryAfterReload<T>(
  entries: readonly T[],
  previousIndex: number,
  isPreviousEntry: (entry: T, index: number) => boolean,
): ReloadEntryBinding {
  if (entries.length === 0) return { index: -1, exact: false };
  const exactIndex = entries.findIndex(isPreviousEntry);
  if (exactIndex >= 0) return { index: exactIndex, exact: true };
  return {
    index: Math.max(0, Math.min(previousIndex, entries.length - 1)),
    exact: false,
  };
}
