/** Java `org.omegat.gui.editor.EditorSettings`. */
export type EditorSettingsState = {
  markWhitespace: boolean;
  markNbsp: boolean;
  markBidi: boolean;
  displaySegmentSources: boolean;
  markTranslated: boolean;
  markUntranslated: boolean;
};

export function defaultEditorSettings(): EditorSettingsState {
  return {
    markWhitespace: true,
    markNbsp: true,
    markBidi: true,
    displaySegmentSources: true,
    markTranslated: true,
    markUntranslated: true,
  };
}
