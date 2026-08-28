import { useState } from "react";
import { t } from "../i18n";
import {
  CommentsController,
  DockNotificationController,
  entryComment,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function CommentsDock() {
  const e = useApp((s) => s.entries[s.index]);
  const [notifyComments, setNotifyComments] = useState(true);
  const controller = new CommentsController<typeof e>();
  const notifications = new DockNotificationController(notifyComments);
  controller.addProvider((entry) => entry ? entryComment(entry) : null, 0);
  const text = e ? controller.render(e) : "";
  return (
    <DockFrame
      title={t("comments")}
      notification={notifications.signal(text ? 1 : 0)}
      menu={[{
        id: "notifications",
        label: "Notifications",
        checked: notifyComments,
        action: () => setNotifyComments((enabled) => !enabled),
      }]}
    >
      <div className="empty-state" style={{ whiteSpace: "pre-wrap" }}>{text || "—"}</div>
    </DockFrame>
  );
}
