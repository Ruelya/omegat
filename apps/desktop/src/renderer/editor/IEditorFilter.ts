/** Java `org.omegat.gui.editor.IEditorFilter`. */
export type IEditorFilter = {
  kind: "untranslated" | "unique" | "noted" | "search" | "none";
  query?: string;
  allowed(entry: { translation: string; note?: string; unique?: boolean }): boolean;
};

export function makeFilter(kind: IEditorFilter["kind"], query?: string): IEditorFilter {
  return {
    kind,
    query,
    allowed(entry) {
      if (kind === "untranslated") return !entry.translation;
      if (kind === "noted") return !!entry.note;
      if (kind === "unique") return entry.unique !== false;
      if (kind === "search") return `${entry.translation}`.includes(query ?? "");
      return true;
    },
  };
}
