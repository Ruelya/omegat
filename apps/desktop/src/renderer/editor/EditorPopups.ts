/** Java `org.omegat.gui.editor.EditorPopups`. */
export type PopupItem = { id: string; label: string };

export function editorPopups(): PopupItem[] {
  return [
    { id: "edit.insert-source", label: "Insert source" },
    { id: "edit.insert-translation", label: "Insert match" },
    { id: "edit.glossary", label: "Add glossary" },
    { id: "edit.register-untranslated", label: "Untranslated" },
  ];
}
