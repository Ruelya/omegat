import { useState } from "react";
import { t } from "../i18n";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function Wizard() {
  const create = useApp((s) => s.create);
  const [root, setRoot] = useState("");
  const [sl, setSl] = useState("en");
  const [tl, setTl] = useState("zh-CN");
  const [seg, setSeg] = useState(true);
  return (
    <Modal id="wizard" title={t("newProject")}>
      <div className="form">
        <label>
          {t("root")}
          <input value={root} onChange={(e) => setRoot(e.target.value)} />
          <button
            type="button"
            onClick={async () => {
              const d = await window.omegat?.pickDir();
              if (d) setRoot(d);
            }}
          >
            {t("openProject")}
          </button>
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
        <div className="btn-row">
          <button
            type="button"
            className="primary"
            onClick={() => void create(root, sl, tl, seg).then(() => useApp.getState().openWindow("wizard", false))}
          >
            {t("create")}
          </button>
          <button type="button" onClick={() => useApp.getState().openWindow("wizard", false)}>
            {t("cancel")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
