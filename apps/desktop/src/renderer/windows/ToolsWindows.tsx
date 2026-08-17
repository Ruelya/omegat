import { useEffect, useState } from "react";
import { t } from "../i18n";
import type { FilterOptionsDto } from "../lib/types";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function AlignWindow() {
  const [src, setSrc] = useState("");
  const [tgt, setTgt] = useState("");
  const [dest, setDest] = useState("");
  const [mode, setMode] = useState("parsewise");
  const [algo, setAlgo] = useState("viterbi");
  const [counter, setCounter] = useState("word");
  return (
    <Modal id="align" title={t("aligner")}>
      <div className="form">
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
        <button
          type="button"
          className="primary"
          onClick={async () => {
            await window.omegat?.rpc("align.run", { source: src, target: tgt, dest, mode, algo, counter });
            useApp.getState().openWindow("align", false);
          }}
        >
          {t("create")}
        </button>
      </div>
    </Modal>
  );
}

export function TeamWindow() {
  const msg = useApp((s) => s.teamMessage);
  const conflicts = useApp((s) => s.teamConflicts);
  const sync = useApp((s) => s.teamSync);
  const resolve = useApp((s) => s.resolveConflict);
  return (
    <Modal id="team" title={t("team")}>
      <p>{msg || "Git / SVN / HTTP / file · prepare → rebase → commit"}</p>
      {conflicts.map((c, i) => (
        <div key={i} className="hit">
          {c.message || c.source}
          <div className="btn-row">
            <button type="button" onClick={() => void resolve("ours")}>{t("keepOurs")}</button>
            <button type="button" onClick={() => void resolve("theirs")}>{t("keepTheirs")}</button>
          </div>
        </div>
      ))}
      <div className="btn-row">
        <button type="button" className="primary" onClick={() => void sync()}>{t("sync")}</button>
        <button type="button" onClick={() => useApp.getState().openWindow("team", false)}>{t("cancel")}</button>
      </div>
    </Modal>
  );
}

export function FiltersWindow() {
  const app = useApp();
  const [opts, setOpts] = useState<FilterOptionsDto | null>(null);
  useEffect(() => {
    void app.loadFilters();
  }, [app]);
  return (
    <Modal id="filters" title={t("filters")} wide>
      {app.filters.map((f) => (
        <div key={f.id} className="hit">
          <button
            type="button"
            onClick={async () => {
              const o = (await window.omegat?.rpc("filters.options", { id: f.id })) as FilterOptionsDto;
              setOpts(o);
            }}
          >
            {f.name}
          </button>
          <span className="meta">{f.masks.join(", ")}</span>
        </div>
      ))}
      {opts && (
        <div className="form">
          <h3>{opts.name}</h3>
          {Object.entries(opts.options).map(([k, v]) => (
            <label key={k}>
              {k}
              <input
                defaultValue={v}
                onBlur={(e) => {
                  void app.patchPrefs({}, { [`filter.${opts.id}.${k}`]: e.target.value });
                }}
              />
            </label>
          ))}
        </div>
      )}
      <button type="button" onClick={() => app.openWindow("filters", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function SegmentationWindow() {
  const extra = useApp((s) => s.prefs?.extra ?? {});
  const patch = useApp((s) => s.patchPrefs);
  const [path, setPath] = useState(extra.srx_path || "fixtures/srx/defaultRules.srx");
  const [xml, setXml] = useState(extra.srx_xml || "");
  return (
    <Modal id="segmentation" title={t("segmentation")} wide>
      <div className="form">
        <label>
          SRX path
          <input value={path} onChange={(e) => setPath(e.target.value)} />
        </label>
        <textarea rows={10} value={xml} onChange={(e) => setXml(e.target.value)} placeholder="<srx>…" />
        <button
          type="button"
          className="primary"
          onClick={() => void patch({}, { srx_path: path, srx_xml: xml })}
        >
          {t("save")}
        </button>
      </div>
    </Modal>
  );
}

const SHORTCUTS: [string, string][] = [
  ["project.save", "CmdOrCtrl+S"],
  ["project.compile", "CmdOrCtrl+D"],
  ["edit.insert-translation", "CmdOrCtrl+I"],
  ["edit.overwrite-translation", "CmdOrCtrl+R"],
  ["goto.untranslated", "CmdOrCtrl+U"],
  ["goto.next", "CmdOrCtrl+N"],
  ["edit.search", "CmdOrCtrl+F"],
  ["edit.replace", "CmdOrCtrl+K"],
];

export function ShortcutsWindow() {
  const extra = useApp((s) => s.prefs?.extra ?? {});
  const patch = useApp((s) => s.patchPrefs);
  return (
    <Modal id="shortcuts" title={t("shortcuts")} wide>
      <table className="stats">
        <tbody>
          {SHORTCUTS.map(([id, def]) => (
            <tr key={id}>
              <td>{id}</td>
              <td>
                <input
                  defaultValue={extra[`shortcut.${id}`] || def}
                  onBlur={(e) => void patch({}, { [`shortcut.${id}`]: e.target.value })}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <button type="button" onClick={() => useApp.getState().openWindow("shortcuts", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function WikiWindow() {
  const [src, setSrc] = useState("");
  return (
    <Modal id="wiki" title={t("wiki")}>
      <input value={src} onChange={(e) => setSrc(e.target.value)} placeholder="page.xml" />
      <button
        type="button"
        className="primary"
        onClick={() => void useApp.getState().importWiki(src).then(() => useApp.getState().openWindow("wiki", false))}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function MedWindow() {
  const [src, setSrc] = useState("");
  const [dest, setDest] = useState("");
  return (
    <Modal id="med" title={t("med")}>
      <input value={src} onChange={(e) => setSrc(e.target.value)} placeholder="pack.zip" />
      <input value={dest} onChange={(e) => setDest(e.target.value)} placeholder="dest" />
      <button
        type="button"
        className="primary"
        onClick={async () => {
          await window.omegat?.rpc("med.open", { source: src, dest });
          useApp.getState().openWindow("med", false);
        }}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function ConvertWindow() {
  const [src, setSrc] = useState("");
  const [dest, setDest] = useState("");
  return (
    <Modal id="convert" title={t("convert")}>
      <input value={src} onChange={(e) => setSrc(e.target.value)} />
      <input value={dest} onChange={(e) => setDest(e.target.value)} />
      <button
        type="button"
        className="primary"
        onClick={async () => {
          await window.omegat?.rpc("project.convert", { source: src, dest, source_lang: "en", target_lang: "fr" });
          useApp.getState().openWindow("convert", false);
        }}
      >
        {t("create")}
      </button>
    </Modal>
  );
}

export function ScriptsWindow() {
  const [src, setSrc] = useState("console.println(editor.getTranslation())");
  const [out, setOut] = useState("");
  return (
    <Modal id="scripts" title={t("scripts")} wide>
      <textarea rows={8} value={src} onChange={(e) => setSrc(e.target.value)} />
      <button
        type="button"
        className="primary"
        onClick={async () => {
          const r = (await window.omegat?.rpc("script.run", { source: src })) as { result?: string };
          setOut(String(r?.result ?? ""));
        }}
      >
        {t("run")}
      </button>
      <pre className="log">{out}</pre>
    </Modal>
  );
}

export function GlossaryAddWindow() {
  const [s, setS] = useState("");
  const [tg, setTg] = useState("");
  return (
    <Modal id="glossary-add" title={t("glossary")}>
      <input value={s} onChange={(e) => setS(e.target.value)} placeholder="source" />
      <input value={tg} onChange={(e) => setTg(e.target.value)} placeholder="target" />
      <button
        type="button"
        className="primary"
        onClick={() => void useApp.getState().addGlossary(s, tg).then(() => useApp.getState().openWindow("glossary-add", false))}
      >
        {t("create")}
      </button>
    </Modal>
  );
}
