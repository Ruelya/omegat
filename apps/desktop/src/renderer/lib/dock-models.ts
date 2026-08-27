/**
 * Compatibility barrel for the original dock-model imports. New desktop
 * code imports the behavior-owning controllers directly.
 */
export {
  decodeGlossaryComment,
  noteText,
  NotesDocument,
  renderGlossaryText,
  type GlossaryDisplayEntry,
} from "./dock-controllers";
