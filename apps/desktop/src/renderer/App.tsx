import { useEffect, useState } from "react";
import {
  Books,
  FolderOpen,
  Gear,
  Moon,
  Sun,
  Translate,
} from "@phosphor-icons/react";
import { availableLocales, t } from "./i18n";
import { useApp } from "./store/app";

export function App() {
  const app = useApp();
  const [wizard, setWizard] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [prefsOpen, setPrefsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [alignOpen, setAlignOpen] = useState(false);
  const [teamOpen, setTeamOpen] = useState(false);
  const [filesOpen, setFilesOpen] = useState(false);
  const [issuesOpen, setIssuesOpen] = useState(false);

  useEffect(() => {
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
      window.omegat?.onMenu("menu:search", () => setSearchOpen(true)),
      window.omegat?.onMenu("menu:prefs", () => setPrefsOpen(true)),
      window.omegat?.onMenu("menu:about", () => setAboutOpen(true)),
      window.omegat?.onMenu("menu:align", () => setAlignOpen(true)),
      window.omegat?.onMenu("menu:issues", () => void app.select(app.index)),
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
            <button type="button" onClick={() => setAlignOpen(true)}>{t("aligner")}</button>
            <button type="button" onClick={() => { void app.teamSync(); setTeamOpen(true); }}>{t("team")}</button>
            <button type="button" onClick={() => setFilesOpen(true)}>{t("files")}</button>
            <button type="button" onClick={() => setIssuesOpen(true)}>{t("issues")}</button>
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
      {alignOpen && <AlignModal onClose={() => setAlignOpen(false)} />}
      {teamOpen && <TeamModal onClose={() => setTeamOpen(false)} />}
      {filesOpen && <FilesModal onClose={() => setFilesOpen(false)} />}
      {issuesOpen && <IssuesModal onClose={() => setIssuesOpen(false)} />}
      {app.firstRun && app.screen === "welcome" && (
        <div className="status">{t("tip")}</div>
      )}
      {app.error && <div className="status">{app.error}</div>}
    </div>
  );
}

function Welcome({ onOpen, onNew }: { onOpen: () => void; onNew: () => void }) {
  const recent = JSON.parse((() => {
    try {
      return localStorage.getItem("omegat.recent") || "[]";
    } catch {
      return "[]";
    }
  })()) as string[];
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
            {t("root")}
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
              onInput={(ev) => {
                const text = (ev.target as HTMLElement).innerText;
                app.setDraft(text);
                void app.queryCompleter(text.split(/\s+/).pop() || "");
              }}
              onKeyDown={(ev) => {
                if (ev.key === "Enter" && !ev.shiftKey) {
                  ev.preventDefault();
                  void app.commit();
                }
                if (ev.key === "Tab" && app.completer[0]) {
                  ev.preventDefault();
                  app.setDraft(app.draft + app.completer[0].text);
                }
                if ((ev.metaKey || ev.ctrlKey) && ev.key === "z") {
                  ev.preventDefault();
                  app.undo();
                }
              }}
            >{app.draft}</div>
            {app.completer.length > 0 && (
              <div className="list" style={{ maxHeight: 80 }}>
                {app.completer.slice(0, 6).map((c, i) => (
                  <div key={i} className="hit" onClick={() => app.setDraft(app.draft + c.text)}>
                    <span className="score">{c.kind}</span> {c.text}
                  </div>
                ))}
              </div>
            )}
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
          <div className="pane-h">{t("multiple")}</div>
          <div className="placeholder">{e?.default_translation ? "default" : "alternate"}</div>
          <div className="pane-h">{t("mt")}</div>
          <div className="list" style={{ maxHeight: "12%" }}>
            {app.mt.map((m, i) => (
              <div key={i} className="hit" onClick={() => app.setDraft(m.text)}>
                <span className="score">{m.engine}</span> {m.text}
              </div>
            ))}
          </div>
          <div className="pane-h">{t("dict")}</div>
          <div className="list" style={{ maxHeight: "10%" }}>
            {app.dict.map((d, i) => (
              <div key={i} className="hit"><b>{d.word}</b> {d.definition}</div>
            ))}
          </div>
          <div className="pane-h">{t("issues")}</div>
          <div className="list" style={{ maxHeight: "16%" }}>
            {app.issues.slice(0, 12).map((iss, i) => (
              <div key={i} className="hit" onClick={() => void app.select(iss.index)}>
                <span className="score">{iss.kind}</span> {iss.message}
                {iss.kind === "spell" && (
                  <>
                    <button type="button" onClick={() => void app.learnWord(iss.message.replace("Unknown word: ", ""))}>{t("learn")}</button>
                    <button type="button" onClick={() => void app.ignoreWord(iss.message.replace("Unknown word: ", ""))}>{t("ignore")}</button>
                  </>
                )}
              </div>
            ))}
            {app.issues.length === 0 && <div className="placeholder">{t("noIssues")}</div>}
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
  const [repl, setRepl] = useState("");
  const [regex, setRegex] = useState(false);
  const [hits, setHits] = useState<{ index: number; file: string; field: string; text: string }[]>([]);
  const select = useApp((s) => s.select);
  const replaceAll = useApp((s) => s.replaceAll);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("search")}</h2>
        <input autoFocus value={q} onChange={async (e) => {
          setQ(e.target.value);
          if (window.omegat && e.target.value) {
            const r = await window.omegat.rpc("search.run", { query: e.target.value, source: true, translation: true, regex }) as typeof hits;
            setHits(r);
          }
        }} />
        <label><input type="checkbox" checked={regex} onChange={(e) => setRegex(e.target.checked)} /> {t("regex")}</label>
        <input placeholder={t("replace")} value={repl} onChange={(e) => setRepl(e.target.value)} />
        <button type="button" onClick={() => void replaceAll(q, repl, regex)}>{t("replace")}</button>
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
  const app = useApp();
  const [page, setPage] = useState("general");
  const [threshold, setThreshold] = useState(30);
  const [autosave, setAutosave] = useState(180);
  const [lt, setLt] = useState("");
  const [extraMap, setExtraMap] = useState<Record<string, string>>({});
  useEffect(() => {
    void app.loadFilters();
    void app.loadPrefs();
  }, [app]);
  const extra = (k: string, v: string) => setExtraMap((m) => ({ ...m, [k]: v }));
  const pages = ["general", "appearance", "save", "editing", "matches", "view", "filters", "segmentation", "spell", "languagetool", "dict", "glossary", "mt", "completer", "finder", "team", "plugins"];
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ width: "min(860px, 94vw)" }}>
        <h2>{t("prefs")}</h2>
        <div style={{ display: "grid", gridTemplateColumns: "180px 1fr", gap: 16 }}>
          <div className="list">
            {pages.map((p) => (
              <div key={p} className={`row ${page === p ? "active" : ""}`} onClick={() => setPage(p)}>{p}</div>
            ))}
          </div>
          <div className="form">
            {page === "general" && (
              <label>
                {t("uiLanguage")}
                <select value={app.locale} onChange={(e) => app.setLocale(e.target.value)}>
                  {availableLocales().map((code) => <option key={code} value={code}>{code}</option>)}
                </select>
              </label>
            )}
            {page === "appearance" && (
              <>
                <label>{t("appearance")}
                  <select value={app.theme} onChange={() => app.toggleTheme()}>
                    <option value="light">light</option>
                    <option value="dark">dark</option>
                  </select>
                </label>
                <label>UI font<input defaultValue="IBM Plex Sans" onChange={(e) => extra("font_ui", e.target.value)} /></label>
                <label>Editor font<input defaultValue="IBM Plex Sans" onChange={(e) => extra("font_editor", e.target.value)} /></label>
              </>
            )}
            {page === "save" && (
              <>
                <label>{t("autosave")}<input type="number" value={autosave} onChange={(e) => setAutosave(Number(e.target.value))} /></label>
                <label>Export TM levels<input defaultValue="omegat level1 level2" onChange={(e) => extra("export_tm_levels", e.target.value)} /></label>
                <label>{t("tagValidation")}
                  <select defaultValue="warn" onChange={(e) => extra("tag_validation", e.target.value)}>
                    <option value="warn">warn</option>
                    <option value="abort">abort</option>
                  </select>
                </label>
              </>
            )}
            {page === "editing" && (
              <>
                <label><input type="checkbox" defaultChecked onChange={(e) => extra("insert_best_match", String(e.target.checked))} /> {t("insertBest")}</label>
                <label><input type="checkbox" onChange={(e) => extra("filter_untranslated", String(e.target.checked))} /> {t("filterUntranslated")}</label>
              </>
            )}
            {page === "matches" && (
              <label>{t("fuzzyThreshold")}<input type="number" value={threshold} onChange={(e) => setThreshold(Number(e.target.value))} /></label>
            )}
            {page === "view" && (
              <>
                <label><input type="checkbox" onChange={(e) => extra("mark_whitespace", String(e.target.checked))} /> {t("markWhitespace")}</label>
                <label><input type="checkbox" onChange={(e) => extra("mark_nbsp", String(e.target.checked))} /> {t("markNbsp")}</label>
                <label><input type="checkbox" onChange={(e) => extra("mark_bidi", String(e.target.checked))} /> {t("markBidi")}</label>
              </>
            )}
            {page === "filters" && app.filters.map((f) => (
              <div key={f.id} className="hit">
                {f.name} <span className="meta">{f.masks.join(", ")}</span>
                <label>preserve spaces <input type="checkbox" defaultChecked onChange={(e) => extra(`filter.${f.id}.preserve_spaces`, String(e.target.checked))} /></label>
              </div>
            ))}
            {page === "segmentation" && (
              <label>SRX path<input defaultValue="fixtures/srx/defaultRules.srx" onChange={(e) => extra("srx_path", e.target.value)} /></label>
            )}
            {page === "languagetool" && (
              <label>LanguageTool URL<input value={lt} onChange={(e) => setLt(e.target.value)} placeholder="http://localhost:8081/v2/check" /></label>
            )}
            {page === "spell" && (
              <label>Backend
                <select defaultValue="hunspell" onChange={(e) => extra("spell_backend", e.target.value)}>
                  <option value="hunspell">Hunspell</option>
                  <option value="lucene">Lucene-Hunspell</option>
                  <option value="morfologik">Morfologik</option>
                </select>
              </label>
            )}
            {page === "dict" && (
              <label>Dictionary folder<input defaultValue="dictionary" onChange={(e) => extra("dictionary_dir", e.target.value)} /></label>
            )}
            {page === "glossary" && (
              <>
                <label><input type="checkbox" defaultChecked onChange={(e) => extra("glossary_stem", String(e.target.checked))} /> {t("glossaryStem")}</label>
                <label><input type="checkbox" defaultChecked onChange={(e) => extra("glossary_ignore_case", String(e.target.checked))} /> ignore case</label>
              </>
            )}
            {page === "mt" && (
              <>
                {["google","ibmwatson","mymemory","mymemory-human","apertium","yandex","belazar"].map((eng) => (
                  <label key={eng}><input type="checkbox" onChange={(e) => extra(`mt.${eng}`, String(e.target.checked))} /> {eng}</label>
                ))}
              </>
            )}
            {page === "completer" && (
              <label>{t("autotext")}<input placeholder="omegat=OmegaT;nbsp=\u00a0" onChange={(e) => extra("autotext", e.target.value)} /></label>
            )}
            {page === "finder" && (
              <label>Finder XML<textarea rows={4} onChange={(e) => extra("finder_xml", e.target.value)} placeholder='<item><name>Wiktionary</name><url>https://en.wiktionary.org/wiki/{selection}</url></item>' /></label>
            )}
            {page === "team" && (
              <label>Passphrase<input type="password" onChange={(e) => extra("team_passphrase", e.target.value)} /></label>
            )}
            {page === "plugins" && (
              <label>Plugin directory<input defaultValue="plugins" onChange={(e) => extra("plugin_dir", e.target.value)} /></label>
            )}
            <button type="button" className="primary" onClick={() => {
              if (app.prefs) {
                void app.savePrefs({
                  ...app.prefs,
                  fuzzy_threshold: threshold,
                  autosave_seconds: autosave,
                  extra: { ...app.prefs.extra, ...extraMap, languagetool_url: lt },
                });
              }
            }}>{t("save")}</button>
          </div>
        </div>
        <button type="button" onClick={onClose}>{t("cancel")}</button>
      </div>
    </div>
  );
}

function AlignModal({ onClose }: { onClose: () => void }) {
  const [src, setSrc] = useState("");
  const [tgt, setTgt] = useState("");
  const [dest, setDest] = useState("");
  const [mode, setMode] = useState("parsewise");
  const [algo, setAlgo] = useState("viterbi");
  const [counter, setCounter] = useState("word");
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("aligner")}</h2>
        <p className="muted">HEAPWISE / PARSEWISE / ID · Viterbi / Forward-Backward · CHAR / WORD</p>
        <input placeholder="source" value={src} onChange={(e) => setSrc(e.target.value)} />
        <input placeholder="target" value={tgt} onChange={(e) => setTgt(e.target.value)} />
        <input placeholder="out.tmx" value={dest} onChange={(e) => setDest(e.target.value)} />
        <select value={mode} onChange={(e) => setMode(e.target.value)}>
          <option value="heapwise">HEAPWISE</option>
          <option value="parsewise">PARSEWISE</option>
          <option value="id">ID</option>
        </select>
        <select value={algo} onChange={(e) => setAlgo(e.target.value)}>
          <option value="viterbi">Viterbi</option>
          <option value="forward-backward">Forward-Backward</option>
        </select>
        <select value={counter} onChange={(e) => setCounter(e.target.value)}>
          <option value="word">WORD</option>
          <option value="char">CHAR</option>
        </select>
        <button type="button" className="primary" onClick={async () => {
          await window.omegat?.rpc("align.run", { source: src, target: tgt, dest, mode, algo, counter });
          onClose();
        }}>{t("create")}</button>
      </div>
    </div>
  );
}

function TeamModal({ onClose }: { onClose: () => void }) {
  const msg = useApp((s) => s.teamMessage);
  const sync = useApp((s) => s.teamSync);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("team")}</h2>
        <p>{msg || "Git / SVN / HTTP / file · prepare → rebase → commit"}</p>
        {msg && msg.includes("conflict") && (
          <p className="muted">{t("conflicts")}: {t("keepOurs")} / {t("keepTheirs")} — both sides stay in the TMX note.</p>
        )}
        <button type="button" className="primary" onClick={() => void sync()}>{t("sync")}</button>
        <button type="button" onClick={onClose}>{t("cancel")}</button>
      </div>
    </div>
  );
}

function FilesModal({ onClose }: { onClose: () => void }) {
  const app = useApp();
  const files = [...new Set(app.entries.map((x) => x.file))];
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("files")}</h2>
        {files.map((f) => (
          <div key={f} className="row" onClick={() => {
            const i = app.entries.findIndex((x) => x.file === f);
            if (i >= 0) void app.select(i);
            onClose();
          }}>
            {f}
            <div className="meta">{app.entries.filter((x) => x.file === f && x.translated).length}/{app.entries.filter((x) => x.file === f).length}</div>
          </div>
        ))}
        <button type="button" onClick={onClose}>{t("cancel")}</button>
      </div>
    </div>
  );
}

function IssuesModal({ onClose }: { onClose: () => void }) {
  const app = useApp();
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{t("issues")}</h2>
        {app.issues.map((iss, i) => (
          <div key={i} className="hit" onClick={() => { void app.select(iss.index); onClose(); }}>
            <span className="score">{iss.kind}</span> {iss.message}
          </div>
        ))}
        {app.issues.length === 0 && <div className="placeholder">{t("noIssues")}</div>}
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
