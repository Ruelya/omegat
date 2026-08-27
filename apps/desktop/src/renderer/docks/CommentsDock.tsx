import { t } from "../i18n";
import { CommentsController, entryComment } from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function CommentsDock() {
  const e = useApp((s) => s.entries[s.index]);
  const controller = new CommentsController<typeof e>();
  controller.addProvider((entry) => entry ? entryComment(entry) : null, 0);
  const text = e ? controller.render(e) : "";
  return (
    <DockFrame title={t("comments")}>
      <div className="empty-state" style={{ whiteSpace: "pre-wrap" }}>{text || "—"}</div>
    </DockFrame>
  );
}
