import { useEffect, useState } from "react";
import { t } from "../i18n";
import { defaultPreferences } from "../lib/preferences";
import type { Preferences } from "../lib/types";
import { useApp } from "../store/app";
import { PREF_PAGES } from "./pages";

export function PrefsWindow() {
  const app = useApp();
  const loadPrefs = app.loadPrefs;
  const [page, setPage] = useState(PREF_PAGES[0]!.id);
  const [draft, setDraft] = useState<Preferences | null>(app.prefs ? defaultPreferences(app.prefs) : null);
  const [saveError, setSaveError] = useState("");
  useEffect(() => {
    void loadPrefs().then(() => {
      const p = useApp.getState().prefs;
      setDraft(p ? defaultPreferences(p) : null);
    });
  }, [loadPrefs]);
  if (!draft) return null;
  const current = PREF_PAGES.find((p) => p.id === page) ?? PREF_PAGES[0]!;
  const setPref = <K extends keyof Preferences>(k: K, v: Preferences[K]) => {
    setDraft({ ...draft, [k]: v });
  };
  const patch = (partial: Partial<Preferences>) => {
    setDraft(defaultPreferences({ ...draft, ...partial }));
  };
  return (
    <div className="modal-bg" onClick={() => app.openWindow("prefs", false)}>
      <div
        className="modal wide"
        data-window-id="prefs"
        onClick={(e) => e.stopPropagation()}
      >
        <h2>{t("prefs")}</h2>
        <div className="prefs-grid">
          <nav className="list">
            {PREF_PAGES.map((p) => (
              <div
                key={p.id}
                className={`row ${page === p.id ? "active" : ""}`}
                data-pref-page={p.id}
                onClick={() => setPage(p.id)}
              >
                {t(p.title)}
              </div>
            ))}
          </nav>
          <div className="form">
            <current.Page prefs={draft} setPref={setPref} patch={patch} />
            <button
              type="button"
              className="primary"
              data-action="save-preferences"
              onClick={async () => {
                setSaveError("");
                try {
                  await app.savePrefs(draft);
                } catch (error) {
                  setSaveError(String(error));
                }
              }}
            >
              {t("save")}
            </button>
            {saveError && (
              <div role="alert" data-persistence-error="prefs">
                {saveError}
              </div>
            )}
          </div>
        </div>
        <button type="button" onClick={() => app.openWindow("prefs", false)}>{t("cancel")}</button>
      </div>
    </div>
  );
}
