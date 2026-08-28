import { useState } from "react";
import { t } from "../i18n";
import {
  DockNotificationController,
  MatchesController,
} from "../lib/dock-controllers";
import { IEditor } from "../editor/IEditor";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MatchesDock() {
  const matches = useApp((s) => s.matches);
  const selected = useApp((s) => s.selectedMatch);
  const openWindow = useApp((s) => s.openWindow);
  const [notifyHits, setNotifyHits] = useState(true);
  const controller = new MatchesController(matches, selected);
  const notifications = new DockNotificationController(notifyHits);
  const select = (index: number) => {
    useApp.setState({ selectedMatch: controller.select(index) });
  };
  return (
    <DockFrame
      title={t("matches")}
      notification={notifications.signal(controller.matches.length)}
      menu={[
        {
          id: "insert",
          label: t("insertTranslation"),
          disabled: controller.getActiveMatch() === null,
          action: () => controller.apply(IEditor, "insert"),
        },
        {
          id: "replace",
          label: t("replace"),
          disabled: controller.getActiveMatch() === null,
          action: () => controller.apply(IEditor, "overwrite"),
        },
        {
          id: "notifications",
          label: "Notifications",
          checked: notifyHits,
          separatorBefore: true,
          action: () => setNotifyHits((enabled) => !enabled),
        },
        {
          id: "preferences",
          label: "Match preferences",
          separatorBefore: true,
          action: () => openWindow("prefs"),
        },
      ]}
    >
      {controller.matches.map((m, i) => (
        <div
          key={`${m.comes_from}-${i}`}
          className={`hit ${i === selected ? "active" : ""}`}
          tabIndex={0}
          onClick={() => select(i)}
          onDoubleClick={() => controller.apply(IEditor, "overwrite", i)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") select(controller.next());
            if (event.key === "ArrowUp") select(controller.previous());
          }}
        >
          <div className="score">
            {m.score}% {m.comes_from}
            {m.adjusted_score != null && m.adjusted_score !== m.score ? ` · adj ${m.adjusted_score}` : ""}
          </div>
          <div className="muted">{m.source}</div>
          <div>{m.translation}</div>
          {i === selected && (
            <div>
              <button type="button" onClick={() => controller.apply(IEditor, "insert", i)}>Insert</button>
              <button type="button" onClick={() => controller.apply(IEditor, "overwrite", i)}>{t("replace")}</button>
            </div>
          )}
        </div>
      ))}
    </DockFrame>
  );
}
