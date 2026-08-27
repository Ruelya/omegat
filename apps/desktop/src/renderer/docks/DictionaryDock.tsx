import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function DictionaryDock() {
  const dict = useApp((s) => s.dict);
  return (
    <DockFrame title={t("dict")}>
      {dict.map((d, i) => (
        <div key={`${d.word}-${i}`} className="hit">
          <b>{d.word}</b> {d.definition}
          <div className="muted">{d.source}</div>
        </div>
      ))}
    </DockFrame>
  );
}
