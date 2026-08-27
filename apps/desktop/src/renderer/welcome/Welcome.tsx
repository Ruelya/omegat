import { FolderOpen, Translate } from "@phosphor-icons/react";
import { t } from "../i18n";
import { useApp } from "../store/app";

export function Welcome() {
  const open = useApp((s) => s.open);
  const recent = JSON.parse((() => {
    try {
      return localStorage.getItem("omegat.recent") || "[]";
    } catch {
      return "[]";
    }
  })()) as string[];
  return (
    <div className="welcome">
      <h1>OmegaT</h1>
      <p>{t("welcomeLead")}</p>
      <div className="cards">
        <button
          type="button"
          className="card"
          onClick={async () => {
            const dir = await window.omegat?.pickDir();
            if (dir) await open(dir);
          }}
        >
          <FolderOpen size={22} />
          <h2>{t("openProject")}</h2>
          <p className="muted">omegat.project</p>
        </button>
        <button type="button" className="card" onClick={() => useApp.getState().openWindow("wizard")}>
          <Translate size={22} />
          <h2>{t("newProject")}</h2>
          <p className="muted">en → fr, source / target</p>
        </button>
      </div>
      {recent.length > 0 && (
        <div style={{ marginTop: 28 }}>
          <div className="pane-h">{t("recent")}</div>
          {recent.map((r) => (
            <div key={r} className="row" onClick={() => void open(r)}>
              <div className="meta">{r}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
