import { useState } from "react";
import { t } from "../i18n";
import {
  MachineTranslateController,
  type DockEditTarget,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MachineTranslationDock() {
  const mt = useApp((s) => s.mt);
  const queryMt = useApp((s) => s.queryMt);
  const draft = useApp((s) => s.draft);
  const setDraft = useApp((s) => s.setDraft);
  const [selected, setSelected] = useState(-1);
  const controller = new MachineTranslateController(mt, selected);
  const editor: DockEditTarget = {
    getCurrentTranslation: () => draft,
    replaceEditText: setDraft,
    insertText: (text) => setDraft(draft + text),
  };
  return (
    <DockFrame title={t("mt")}>
      <button type="button" onClick={() => void queryMt()}>
        {t("fetchMt")}
      </button>
      <button
        type="button"
        onClick={() => {
          controller.cycle();
          setSelected(controller.selectedIndex);
        }}
      >
        Next
      </button>
      {controller.results.map((m, i) => (
        <div
          key={`${m.engine}-${i}`}
          className={`hit ${i === controller.selectedIndex ? "active" : ""}`}
          tabIndex={0}
          onClick={() => setSelected(controller.select(i))}
          onDoubleClick={() => controller.apply(editor, "overwrite", i)}
        >
          <span className="score">{m.engine}</span> {m.text}
          {i === controller.selectedIndex && (
            <div>
              <button type="button" onClick={() => controller.apply(editor, "insert", i)}>Insert</button>
              <button type="button" onClick={() => controller.apply(editor, "overwrite", i)}>{t("replace")}</button>
            </div>
          )}
        </div>
      ))}
    </DockFrame>
  );
}
