import type { MouseEvent } from "react";
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
          {app.prefs?.project_files_show_translation_progress !== false && (
            <div className="meta">
              {app.entries.filter((x) => x.file === f && x.translated).length}/{app.entries.filter((x) => x.file === f).length}
            </div>
          )}
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
  const persistSpellWord = async (
    event: MouseEvent<HTMLButtonElement>,
    action: "learn" | "ignore",
    word: string,
  ) => {
    event.stopPropagation();
    useApp.setState({ error: null });
    try {
      if (action === "learn") await app.learnWord(word);
      else await app.ignoreWord(word);
    } catch (error) {
      useApp.setState({ error: `spell persistence failed: ${String(error)}` });
    }
  };
  return (
    <Modal id="issues" title={t("issues")} wide>
      {issues.map((iss, i) => (
        <div
          key={`${iss.index}-${i}`}
          className="hit"
          data-issue-index={iss.index}
          data-issue-file={iss.file}
          data-issue-kind={iss.kind}
          onClick={() => {
            void app.select(iss.index).then(() => app.openWindow("issues", false));
          }}
        >
          <span className="score">{iss.kind}</span> {iss.message}
          {iss.kind === "spell" && (
            <>
              <button
                type="button"
                data-action="spell-learn"
                data-spell-word={iss.message.replace("Unknown word: ", "")}
                onClick={(event) =>
                  void persistSpellWord(
                    event,
                    "learn",
                    iss.message.replace("Unknown word: ", ""),
                  )}
              >
                {t("learn")}
              </button>
              <button
                type="button"
                data-action="spell-ignore"
                data-spell-word={iss.message.replace("Unknown word: ", "")}
                onClick={(event) =>
                  void persistSpellWord(
                    event,
                    "ignore",
                    iss.message.replace("Unknown word: ", ""),
                  )}
              >
                {t("ignore")}
              </button>
            </>
          )}
        </div>
      ))}
      {issues.length === 0 && <div className="empty-state">{t("noIssues")}</div>}
      {app.error?.startsWith("spell persistence failed:") && (
        <div role="alert" data-persistence-error="spell">{app.error}</div>
      )}
      <div className="muted">{fileOnly}</div>
      <button type="button" onClick={() => app.openWindow("issues", false)}>{t("cancel")}</button>
    </Modal>
  );
}
