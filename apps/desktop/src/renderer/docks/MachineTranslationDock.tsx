import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MachineTranslationDock() {
  const mt = useApp((s) => s.mt);
  const insertMt = useApp((s) => s.insertMt);
  const queryMt = useApp((s) => s.queryMt);
  return (
    <DockFrame title={t("mt")}>
      <button type="button" onClick={() => void queryMt()}>
        {t("fetchMt")}
      </button>
      {mt.map((m, i) => (
        <div key={`${m.engine}-${i}`} className="hit" onClick={() => insertMt("overwrite")}>
          <span className="score">{m.engine}</span> {m.text}
        </div>
      ))}
    </DockFrame>
  );
}
