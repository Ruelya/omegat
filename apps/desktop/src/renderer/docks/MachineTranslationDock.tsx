import { useState } from "react";
import { t } from "../i18n";
import {
  DockNotificationController,
  MachineTranslateController,
} from "../lib/dock-controllers";
import { IEditor } from "../editor/IEditor";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MachineTranslationDock() {
  const mt = useApp((s) => s.mt);
  const queryMt = useApp((s) => s.queryMt);
  const openWindow = useApp((s) => s.openWindow);
  const [selected, setSelected] = useState(-1);
  const [notifyHits, setNotifyHits] = useState(true);
  const controller = new MachineTranslateController(mt, selected);
  const notifications = new DockNotificationController(notifyHits);
  return (
    <DockFrame
      title={t("mt")}
      notification={notifications.signal(controller.results.length)}
      menu={[
        {
          id: "fetch",
          label: t("fetchMt"),
          action: () => void queryMt().catch(() => undefined),
        },
        {
          id: "insert",
          label: t("insertTranslation"),
          disabled: controller.getSelected() === null,
          action: () => controller.apply(IEditor, "insert"),
        },
        {
          id: "replace",
          label: t("replace"),
          disabled: controller.getSelected() === null,
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
          label: "Machine translation preferences",
          separatorBefore: true,
          action: () => openWindow("prefs"),
        },
      ]}
    >
      <button
        type="button"
        data-mt-fetch
        onClick={() => void queryMt().catch(() => undefined)}
      >
        {t("fetchMt")}
      </button>
      <button
        type="button"
        onClick={() => {
          controller.cycle();
          setSelected(controller.selectedIndex);
        }}
      >
        Next
      </button>
      {controller.results.map((m, i) => (
        <div
          key={`${m.engine}-${i}`}
          data-mt-result={m.engine}
          className={`hit ${i === controller.selectedIndex ? "active" : ""}`}
          tabIndex={0}
          onClick={() => setSelected(controller.select(i))}
          onDoubleClick={() => controller.apply(IEditor, "overwrite", i)}
        >
          <span className="score">{m.engine}</span> {m.text}
          {i === controller.selectedIndex && (
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
