import { useEffect, useState } from "react";
import { t } from "../i18n";
import type { Preferences } from "../lib/types";
import { useApp } from "../store/app";
import { PREF_PAGES } from "./pages";

export function PrefsWindow() {
  const app = useApp();
  const [page, setPage] = useState(PREF_PAGES[0]!.id);
  const [draft, setDraft] = useState<Preferences | null>(app.prefs);
  useEffect(() => {
    void app.loadPrefs().then(() => setDraft(useApp.getState().prefs));
  }, [app]);
  if (!draft) return null;
  const current = PREF_PAGES.find((p) => p.id === page) ?? PREF_PAGES[0]!;
  const setPref = <K extends keyof Preferences>(k: K, v: Preferences[K]) => {
    setDraft({ ...draft, [k]: v });
  };
  const setExtra = (k: string, v: string) => {
    setDraft({ ...draft, extra: { ...draft.extra, [k]: v } });
  };
  return (
    <div className="modal-bg" onClick={() => app.openWindow("prefs", false)}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h2>{t("prefs")}</h2>
        <div className="prefs-grid">
          <nav className="list">
            {PREF_PAGES.map((p) => (
              <div key={p.id} className={`row ${page === p.id ? "active" : ""}`} onClick={() => setPage(p.id)}>
                {t(p.title)}
              </div>
            ))}
          </nav>
          <div className="form">
            <current.Page prefs={draft} extra={draft.extra} setPref={setPref} setExtra={setExtra} />
            <button
              type="button"
              className="primary"
              onClick={() => {
                void app.savePrefs(draft);
                if (draft.locale !== app.locale) app.setLocale(draft.locale);
              }}
            >
              {t("save")}
            </button>
          </div>
        </div>
        <button type="button" onClick={() => app.openWindow("prefs", false)}>{t("cancel")}</button>
      </div>
    </div>
  );
}
