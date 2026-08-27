/** Java `org.omegat.gui.editor.TranslationUndoManager`. */
export class TranslationUndoManager {
  undoStack: string[] = [];
  redoStack: string[] = [];
  remember(text: string) {
    this.undoStack.push(text);
    this.redoStack = [];
  }
  undo(current: string): string {
    const prev = this.undoStack.pop();
    if (prev === undefined) return current;
    this.redoStack.push(current);
    return prev;
  }
  redo(current: string): string {
    const next = this.redoStack.pop();
    if (next === undefined) return current;
    this.undoStack.push(current);
    return next;
  }
}
