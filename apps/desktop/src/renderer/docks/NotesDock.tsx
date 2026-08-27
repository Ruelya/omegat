import { useEffect, useRef, useState } from "react";
import { t } from "../i18n";
import {
  DockNotificationController,
  NotesController,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function NotesDock() {
  const note = useApp((s) => s.note);
  const index = useApp((s) => s.index);
  const setNote = useApp((s) => s.setNote);
  const focus = useApp((s) => s.focusPanel);
  const [notifyNotes, setNotifyNotes] = useState(true);
  const controller = useRef(new NotesController(note));
  const notifications = new DockNotificationController(notifyNotes);
  useEffect(() => {
    controller.current.activate(note);
  }, [index]);
  return (
    <DockFrame
      title={t("notes")}
      notification={notifications.signal(note ? 1 : 0)}
      menu={[{
        id: "notifications",
        label: "Notifications",
        checked: notifyNotes,
        action: () => setNotifyNotes((enabled) => !enabled),
      }]}
    >
      <textarea
        autoFocus={focus === "notes"}
        value={note}
        onChange={(event) => {
          controller.current.set(event.target.value);
          setNote(controller.current.get() ?? "");
        }}
        onKeyDown={(event) => {
          if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "z") return;
          event.preventDefault();
          const value = event.shiftKey
            ? controller.current.redo()
            : controller.current.undo();
          setNote(value ?? "");
        }}
        rows={6}
      />
    </DockFrame>
  );
}
