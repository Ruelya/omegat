import { t } from "../i18n";
import { noteText } from "../lib/dock-models";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function NotesDock() {
  const note = useApp((s) => s.note);
  const setNote = useApp((s) => s.setNote);
  const focus = useApp((s) => s.focusPanel);
  return (
    <DockFrame title={t("notes")}>
      <textarea
        autoFocus={focus === "notes"}
        value={note}
        onChange={(e) => setNote(noteText(e.target.value) ?? "")}
        rows={6}
      />
    </DockFrame>
  );
}
