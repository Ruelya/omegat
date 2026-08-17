import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function CommentsDock() {
  const e = useApp((s) => s.entries[s.index]);
  return (
    <DockFrame title={t("comments")}>
      <div className="placeholder">{e?.comment || "—"}</div>
    </DockFrame>
  );
}
