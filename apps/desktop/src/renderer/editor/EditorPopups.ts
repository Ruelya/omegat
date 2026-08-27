/** Java `org.omegat.gui.editor.EditorPopups` constructors. */
export type PopupItem = { id: string; label: string };

/** Same menu ids Java builds on the editor context menu. */
export function editorPopups(): PopupItem[] {
  return [
    { id: "ccp.cut", label: "Cut" },
    { id: "ccp.copy", label: "Copy" },
    { id: "ccp.paste", label: "Paste" },
    { id: "spell.ignore-all", label: "Ignore all" },
    { id: "spell.add-to-dictionary", label: "Add to dictionary" },
    { id: "glossary.add-entry", label: "Add glossary entry" },
    { id: "glossary.change", label: "Change glossary" },
    { id: "goto.segment", label: "Go to segment" },
    { id: "goto.duplicate", label: "Go to duplicate" },
    { id: "trans.empty", label: "Empty translation" },
    { id: "trans.remove", label: "Remove translation" },
    { id: "trans.identical", label: "Set identical translation" },
    { id: "edit.insert-tag", label: "Insert tag" },
    { id: "edit.insert-chars", label: "Insert characters" },
    { id: "edit.insert-source", label: "Insert source" },
    { id: "edit.insert-translation", label: "Insert match" },
  ];
}
