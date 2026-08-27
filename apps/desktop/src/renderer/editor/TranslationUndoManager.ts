/** Java `org.omegat.gui.editor.TranslationUndoManager`. */
export class TranslationUndoManager<T = string> {
  undoStack: T[] = [];
  redoStack: T[] = [];
  remember(state: T) {
    this.undoStack.push(state);
    this.redoStack = [];
  }
  undo(current: T): T {
    const prev = this.undoStack.pop();
    if (prev === undefined) return current;
    this.redoStack.push(current);
    return prev;
  }
  redo(current: T): T {
    const next = this.redoStack.pop();
    if (next === undefined) return current;
    this.undoStack.push(current);
    return next;
  }
}
