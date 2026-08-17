import { useEffect, useState } from "react";
import {
  Books,
  FolderOpen,
  Gear,
  Moon,
  Sun,
  Translate,
} from "@phosphor-icons/react";
import { t, setLocale } from "./i18n";
import { useApp } from "./store/app";

export function App() {
  const app = useApp();
  const [wizard, setWizard] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [prefsOpen, setPrefsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);

  useEffect(() => {
    const nav = navigator.language || "en";
    if (nav.startsWith("zh")) setLocale("zh-CN");
    app.loadVersion();
    const offs = [
      window.omegat?.onMenu("menu:open", (p) => {
        if (typeof p === "string") void app.open(p);
      }),
      window.omegat?.onMenu("menu:save", () => void app.save()),
      window.omegat?.onMenu("menu:compile", () => void app.compile()),
      window.omegat?.onMenu("menu:insert-match", () => app.insertBest()),
      window.omegat?.onMenu("menu:next", () => void app.select(app.index + 1)),
      window.omegat?.onMenu("menu:prev", () => void app.select(Math.max(0, app.index - 1))),
    ];
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey && app.screen === "workspace") {
        if ((e.target as HTMLElement)?.getAttribute("contenteditable") === "true") {
          e.preventDefault();
          void app.commit();
        }
      }
      if (e.key === "f" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setSearchOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      offs.forEach((off) => off?.());
      window.removeEventListener("keydown", onKey);
    };
  }, [app]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Omega<span>T</span>
        </div>
        <span className="muted">{app.version && `v${app.version}`}</span>
        <div style={{ flex: 1 }} />
        {app.screen === "workspace" && (
          <>
            <button type="button" onClick={() => void app.save()}>
              {t("save")}
            </button>
            <button type="button" className="primary" onClick={() => void app.compile()}>
              {t("compile")}
            </button>
            <button type="button" onClick={() => setSearchOpen(true)}>
              {t("search")}
            </button>
          </>
        )}
        <button type="button" onClick={() => setPrefsOpen(true)} aria-label={t("prefs")}>
          <Gear size={16} />
        </button>
        <button type="button" onClick={() => setAboutOpen(true)} aria-label={t("about")}>
          <Books size={16} />
        </button>
        <button type="button" onClick={app.toggleTheme} aria-label="theme">
          {app.theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
        </button>
      </header>
      {app.screen === "welcome" ? (
        <Welcome onOpen={async () => {
          const dir = await window.omegat?.pickDir();
          if (dir) await app.open(dir);
        }} onNew={() => setWizard(true)} />
      ) : (
        <Workspace />
      )}
      {wizard && <Wizard onClose={() => setWizard(false)} />}
      {searchOpen && <SearchModal onClose={() => setSearchOpen(false)} />}
      {prefsOpen && <PrefsModal onClose={() => setPrefsOpen(false)} />}
      {aboutOpen && <AboutModal onClose={() => setAboutOpen(false)} />}
      {app.firstRun && app.screen === "welcome" && (
        <div className="status">{t("tip")}</div>
      )}
      {app.error && <div className="status">{app.error}</div>}
    </div>
  );
}

function Welcome({ onOpen, onNew }: { onOpen: () => void; onNew: () => void }) {
  const recent = JSON.parse(localStorage.getItem("omegat.recent") || "[]") as string[];
  const open = useApp((s) => s.open);
  return (
    <div className="welcome">
      <h1>OmegaT</h1>
      <p>{t("welcomeLead")}</p>
      <div className="cards">
        <button type="button" className="card" onClick={onOpen}>
          <FolderOpen size={22} />
          <h2>{t("openProject")}</h2>
          <p className="muted">omegat.project</p>
        </button>
        <button type="button" className="card" onClick={onNew}>
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

function Wizard({ onClose }: { onClose: () => void }) {
  const create = useApp((s) => s.create);
  const [root, setRoot] = useState("");
  const [sl, setSl] = useState("en");
  const [tl, setTl] = useState("zh-CN");
  const [seg, setSeg] = useState(true);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("newProject")}</h2>
        <div className="form">
          <label>
            Root
            <input value={root} onChange={(e) => setRoot(e.target.value)} />
            <button type="button" onClick={async () => {
              const d = await window.omegat?.pickDir();
              if (d) setRoot(d);
            }}>{t("openProject")}</button>
          </label>
          <label>
            {t("sourceLang")}
            <input value={sl} onChange={(e) => setSl(e.target.value)} />
          </label>
          <label>
            {t("targetLang")}
            <input value={tl} onChange={(e) => setTl(e.target.value)} />
          </label>
          <label>
            <input type="checkbox" checked={seg} onChange={(e) => setSeg(e.target.checked)} /> {t("sentenceSeg")}
          </label>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" className="primary" onClick={() => void create(root, sl, tl, seg).then(onClose)}>{t("create")}</button>
            <button type="button" onClick={onClose}>{t("cancel")}</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function Workspace() {
  const app = useApp();
  const e = app.entries[app.index];
  const files = [...new Set(app.entries.map((x) => x.file))];
  return (
    <>
      <div className="workspace">
        <div className="col">
          <div className="pane-h">{t("files")}</div>
          <div className="list">
            {files.map((f) => (
              <div key={f} className="row" onClick={() => {
                const i = app.entries.findIndex((x) => x.file === f);
                if (i >= 0) void app.select(i);
              }}>
                {f}
                <div className="meta">{app.entries.filter((x) => x.file === f && x.translated).length}/{app.entries.filter((x) => x.file === f).length}</div>
              </div>
            ))}
            {app.entries.map((seg) => (
              <div key={seg.index} className={`row ${seg.index === app.index ? "active" : ""}`} onClick={() => void app.select(seg.index)}>
                <div className="meta">#{seg.index + 1} {seg.translated ? "●" : "○"}</div>
                {seg.source.slice(0, 80)}
              </div>
            ))}
          </div>
        </div>
        <div className="col">
          <div className="pane-h">{t("editor")} {e ? `#${e.index + 1}` : ""}</div>
          <div className="editor">
            <div className="pane-h">{t("source")}</div>
            <div className="src">{e?.source}
              <div>{e?.tags.map((tg) => <span key={tg} className="tag">{tg}</span>)}</div>
            </div>
            <div className="pane-h">{t("target")}</div>
            <div
              className="tgt"
              contentEditable
              suppressContentEditableWarning
              onInput={(ev) => app.setDraft((ev.target as HTMLElement).innerText)}
              onKeyDown={(ev) => {
                if (ev.key === "Enter" && !ev.shiftKey) {
                  ev.preventDefault();
                  void app.commit();
                }
              }}
            >{app.draft}</div>
          </div>
        </div>
        <div className="col">
          <div className="pane-h">{t("matches")}</div>
          <div className="list" style={{ maxHeight: "28%" }}>
            {app.matches.map((m, i) => (
              <div key={i} className="hit" onClick={() => app.setDraft(m.translation)}>
                <div className="score">{m.score}% {m.comes_from}</div>
                <div className="muted">{m.source}</div>
                <div>{m.translation}</div>
              </div>
            ))}
          </div>
          <div className="pane-h">{t("glossary")}</div>
          <div className="list" style={{ maxHeight: "18%" }}>
            {app.glossary.map((g, i) => (
              <div key={i} className="hit" onClick={() => app.setDraft(app.draft + g.target)}>
                <b>{g.source}</b> → {g.target}
              </div>
            ))}
          </div>
          <div className="pane-h">{t("notes")}</div>
          <textarea value={app.note} onChange={(e) => useApp.setState({ note: e.target.value })} rows={3} />
          <div className="pane-h">{t("comments")}</div>
          <div className="placeholder">{e?.comment || "—"}</div>
          <div className="pane-h">{t("properties")}</div>
          <div className="placeholder">{e ? `${e.file} · ${e.id} · rev ${e.revision}` : ""}</div>
          <div className="pane-h">{t("mt")} / {t("dict")} / {t("issues")}</div>
          <div className="list" style={{ maxHeight: "22%" }}>
            {app.issues.slice(0, 12).map((iss, i) => (
              <div key={i} className="hit" onClick={() => void app.select(iss.index)}>
                <span className="score">{iss.kind}</span> {iss.message}
              </div>
            ))}
            {app.issues.length === 0 && <div className="placeholder">{t("comingLater")}</div>}
          </div>
        </div>
      </div>
      <footer className="status">
        <span>{app.stats ? `${app.stats.translated}/${app.stats.segments}` : ""}</span>
        <span>{app.props ? `${app.props.source_lang} → ${app.props.target_lang}` : ""}</span>
        <span>{app.props?.root}</span>
      </footer>
    </>
  );
}

function SearchModal({ onClose }: { onClose: () => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<{ index: number; file: string; field: string; text: string }[]>([]);
  const select = useApp((s) => s.select);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("search")}</h2>
        <input autoFocus value={q} onChange={async (e) => {
          setQ(e.target.value);
          if (window.omegat && e.target.value) {
            const r = await window.omegat.rpc("search.run", { query: e.target.value, source: true, translation: true }) as typeof hits;
            setHits(r);
          }
        }} />
        {hits.map((h, i) => (
          <div key={i} className="hit" onClick={() => { void select(h.index); onClose(); }}>
            <span className="meta">#{h.index} {h.field}</span> {h.text}
          </div>
        ))}
      </div>
    </div>
  );
}

function PrefsModal({ onClose }: { onClose: () => void }) {
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("prefs")}</h2>
        <p className="muted">General · Appearance · Save · Editing · TM matches · View</p>
        <p className="muted">File Filters · Segmentation · Spellchecker · LanguageTool · Dictionary · Glossary · MT · Autocompleter · External Finder · Team · Plugins</p>
        <button type="button" onClick={onClose}>{t("cancel")}</button>
      </div>
    </div>
  );
}

function AboutModal({ onClose }: { onClose: () => void }) {
  const version = useApp((s) => s.version);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>OmegaT {version}</h2>
        <p>GNU GPL v3+. Rewrite: React + Vite + Electron / Rust sidecar.</p>
        <p className="muted">https://omegat.org</p>
        <button type="button" onClick={onClose}>{t("cancel")}</button>
      </div>
    </div>
  );
}
