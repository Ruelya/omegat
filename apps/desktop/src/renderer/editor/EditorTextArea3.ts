/** Java `org.omegat.gui.editor.EditorTextArea3`. */
import { createDocument3, type Document3State } from "./Document3";
import { deleteBackwardAtomic } from "../lib/editor-doc";

export class EditorTextArea3 {
  doc: Document3State;
  constructor(source = "", translation = "") {
    this.doc = createDocument3(source, translation);
  }
  getText() {
    return this.doc.translation;
  }
  setText(text: string) {
    this.doc = { ...this.doc, translation: text, activeEnd: text.length, dirty: true };
  }
  deleteBackward() {
    this.doc = { ...this.doc, translation: deleteBackwardAtomic(this.doc.translation), dirty: true };
  }
}
