/** Java `org.omegat.gui.editor.EditorController` — IEditor implementation host. */
import { IEditor } from "./IEditor";
import { MarkerController } from "./MarkerController";
import { TranslationUndoManager } from "./TranslationUndoManager";

export class EditorController {
  readonly editor = IEditor;
  readonly markers = new MarkerController();
  readonly undo = new TranslationUndoManager();

  getCurrentTranslation() {
    return this.editor.getCurrentTranslation();
  }
  replaceEditText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.replaceEditText(text);
  }
  insertText(text: string) {
    this.undo.remember(this.getCurrentTranslation());
    this.editor.insertText(text);
  }
  commitAndDeactivate() {
    return this.editor.commitAndDeactivate();
  }
}
