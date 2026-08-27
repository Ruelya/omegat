import { SegmentEditor, SegmentSource } from "../editor/SegmentEditor";
import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function EditorDock() {
  const e = useApp((s) => s.entries[s.index]);
  const marks = useApp((s) => s.marks);
  const changer = e?.properties.find(([k]) => k === "changeid")?.[1];
  const changed = e?.properties.find(([k]) => k === "changedate")?.[1];
  return (
    <DockFrame title={`${t("editor")} ${e ? `#${e.index + 1}` : ""}`}>
      <div className="editor">
        <div className="pane-h">{t("source")}</div>
        <SegmentSource />
        <SegmentEditor />
        {marks.modification !== "none" && e && (
          <div className="mod-info">
            {changer || "—"} {changed || ""}
          </div>
        )}
      </div>
    </DockFrame>
  );
}
