import { t } from "../i18n";
import { SegmentPropertiesController } from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function SegmentPropertiesDock() {
  const e = useApp((s) => s.entries[s.index]);
  const controller = new SegmentPropertiesController(["hasNote", "hasComment"]);
  if (!e) return <DockFrame title={t("properties")}><div className="empty-state">—</div></DockFrame>;
  const rows = controller.rows(e);
  return (
    <DockFrame title={t("properties")}>
      <div className="prop-grid">
        {rows.map((row) => (
          <div key={row.key} className={row.notify ? "active" : ""}>
            <span>{row.key}</span>
            <b>{row.value}</b>
          </div>
        ))}
      </div>
    </DockFrame>
  );
}
