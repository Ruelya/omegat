// Migrated from Groovy event scripts. Runs on ENTRY_ACTIVATED.
// Bindings: project, editor, glossary, console, mainWindow, Core
if (editor.getCurrentSource() && !editor.getCurrentTranslation()) {
  console.println("untranslated: " + editor.getCurrentSource());
}
