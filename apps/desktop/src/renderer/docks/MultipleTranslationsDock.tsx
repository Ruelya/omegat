import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MultipleTranslationsDock() {
  const e = useApp((s) => s.entries[s.index]);
  const same = useApp((s) => s.entries.filter((x) => e && x.source === e.source));
  return (
    <DockFrame title={t("multiple")}>
      <div className="empty-state">{e?.default_translation ? t("defaultTranslation") : t("alternateTranslation")}</div>
      {same.map((x) => (
        <div key={x.index} className="hit">
          #{x.index + 1} {x.file} — {x.translation || "—"}
        </div>
      ))}
    </DockFrame>
  );
}
