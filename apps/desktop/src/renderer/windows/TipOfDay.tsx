import { t } from "../i18n";
import { TIPS } from "../tips/en";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function TipOfDay() {
  const i = useApp((s) => s.tipIndex);
  const tip = TIPS[i % TIPS.length]!;
  return (
    <Modal id="tip" title={t("tipOfDay")}>
      <h3>{tip.name}</h3>
      <p>{tip.body}</p>
      <div className="btn-row">
        <button type="button" onClick={() => useApp.setState({ tipIndex: i + 1 })}>
          {t("nextTip")}
        </button>
        <button type="button" onClick={() => useApp.getState().openWindow("tip", false)}>
          {t("cancel")}
        </button>
      </div>
    </Modal>
  );
}
