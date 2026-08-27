import { useState } from "react";
import { t } from "../i18n";
import {
  decodeGlossaryComment,
  DockNotificationController,
  GlossaryController,
  type DockEditTarget,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function GlossaryDock() {
  const glossary = useApp((s) => s.glossary);
  const setDraft = useApp((s) => s.setDraft);
  const draft = useApp((s) => s.draft);
  const openWindow = useApp((s) => s.openWindow);
  const [selected, setSelected] = useState(0);
  const [notifyHits, setNotifyHits] = useState(true);
  const controller = new GlossaryController(glossary);
  const notifications = new DockNotificationController(notifyHits);
  const editor: DockEditTarget = {
    getCurrentTranslation: () => draft,
    replaceEditText: setDraft,
    insertText: (text) => setDraft(draft + text),
  };
  return (
    <DockFrame
      title={t("glossary")}
      notification={notifications.signal(controller.entries.length)}
      menu={[
        {
          id: "insert",
          label: t("insertTranslation"),
          disabled: controller.entries[selected] === undefined,
          action: () => controller.insertTarget(editor, selected),
        },
        {
          id: "add",
          label: t("createGlossary"),
          action: () => openWindow("glossary-add"),
        },
        {
          id: "notifications",
          label: "Notifications",
          checked: notifyHits,
          separatorBefore: true,
          action: () => setNotifyHits((enabled) => !enabled),
        },
      ]}
    >
      <span className="sr-only" aria-label={controller.getText()} />
      {controller.entries.map((g, i) => (
        <div
          key={`${g.source}-${i}`}
          className={`hit ${i === selected ? "active" : ""}`}
          onClick={() => setSelected(i)}
        >
          <b>{g.source}</b> → {g.target}
          {g.comment && <div className="muted">{decodeGlossaryComment(g.comment)}</div>}
          <button type="button" onClick={() => controller.insertTarget(editor, i)}>Insert</button>
        </div>
      ))}
    </DockFrame>
  );
}
