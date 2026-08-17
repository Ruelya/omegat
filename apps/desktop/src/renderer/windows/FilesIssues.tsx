import { t } from "../i18n";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function FilesWindow() {
  const app = useApp();
  const files = [...new Set(app.entries.map((x) => x.file))];
  return (
    <Modal id="files" title={t("files")}>
      {files.map((f) => (
        <div
          key={f}
          className="row"
          onClick={() => {
            const i = app.entries.findIndex((x) => x.file === f);
            if (i >= 0) void app.select(i);
            app.openWindow("files", false);
          }}
        >
          {f}
          <div className="meta">
            {app.entries.filter((x) => x.file === f && x.translated).length}/{app.entries.filter((x) => x.file === f).length}
          </div>
        </div>
      ))}
      <button type="button" onClick={() => app.openWindow("files", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function IssuesWindow() {
  const app = useApp();
  const fileOnly = app.windows.issues && app.status;
  const issues = app.issues;
  return (
    <Modal id="issues" title={t("issues")} wide>
      {issues.map((iss, i) => (
        <div
          key={`${iss.index}-${i}`}
          className="hit"
          onClick={() => {
            void app.select(iss.index);
            app.openWindow("issues", false);
          }}
        >
          <span className="score">{iss.kind}</span> {iss.message}
          {iss.kind === "spell" && (
            <>
              <button type="button" onClick={() => void app.learnWord(iss.message.replace("Unknown word: ", ""))}>{t("learn")}</button>
              <button type="button" onClick={() => void app.ignoreWord(iss.message.replace("Unknown word: ", ""))}>{t("ignore")}</button>
            </>
          )}
        </div>
      ))}
      {issues.length === 0 && <div className="placeholder">{t("noIssues")}</div>}
      <div className="muted">{fileOnly}</div>
      <button type="button" onClick={() => app.openWindow("issues", false)}>{t("cancel")}</button>
    </Modal>
  );
}
