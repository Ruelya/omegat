import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function SegmentPropertiesDock() {
  const e = useApp((s) => s.entries[s.index]);
  if (!e) return <DockFrame title={t("properties")}><div className="empty-state">—</div></DockFrame>;
  return (
    <DockFrame title={t("properties")}>
      <div className="prop-grid">
        <span>file</span><span>{e.file}</span>
        <span>id</span><span>{e.id}</span>
        <span>rev</span><span>{e.revision}</span>
        {e.properties.map(([k, v]) => (
          <span key={k}>{k}<b>{v}</b></span>
        ))}
      </div>
    </DockFrame>
  );
}
