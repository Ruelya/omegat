import { useRef, useState } from "react";
import { t } from "../i18n";
import { SegmentPropertiesController } from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function SegmentPropertiesDock() {
  const e = useApp((s) => s.entries[s.index]);
  const controller = useRef(new SegmentPropertiesController(["hasNote", "hasComment"]));
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [, setRevision] = useState(0);
  if (!e) return <DockFrame title={t("properties")}><div className="empty-state">—</div></DockFrame>;
  const rows = controller.current.rows(e);
  const menuKey = selectedKey ?? rows[0]?.key;
  const menuRow = rows.find((row) => row.key === menuKey);
  return (
    <DockFrame
      title={t("properties")}
      notification={controller.current.notifiedRowIndices(e).length > 0 ? "hit" : null}
      menu={[{
        id: "notifications",
        label: menuKey ? `Notify on ${menuKey}` : "Notify on property",
        checked: menuRow?.notify ?? false,
        disabled: menuKey === undefined,
        action: () => {
          if (!menuKey) return;
          controller.current.toggleNotification(menuKey, !menuRow?.notify);
          setRevision((revision) => revision + 1);
        },
      }]}
    >
      <div className="prop-grid">
        {rows.map((row) => (
          <div
            key={row.key}
            className={row.notify ? "active" : ""}
            onContextMenu={() => setSelectedKey(row.key)}
          >
            <span>{row.key}</span>
            <b>{row.value}</b>
          </div>
        ))}
      </div>
    </DockFrame>
  );
}
