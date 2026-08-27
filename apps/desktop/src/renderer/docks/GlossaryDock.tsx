import { t } from "../i18n";
import {
  decodeGlossaryComment,
  GlossaryController,
  type DockEditTarget,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function GlossaryDock() {
  const glossary = useApp((s) => s.glossary);
  const setDraft = useApp((s) => s.setDraft);
  const draft = useApp((s) => s.draft);
  const controller = new GlossaryController(glossary);
  const editor: DockEditTarget = {
    getCurrentTranslation: () => draft,
    replaceEditText: setDraft,
    insertText: (text) => setDraft(draft + text),
  };
  return (
    <DockFrame title={t("glossary")}>
      <span className="sr-only" aria-label={controller.getText()} />
      {controller.entries.map((g, i) => (
        <div key={`${g.source}-${i}`} className="hit">
          <b>{g.source}</b> → {g.target}
          {g.comment && <div className="muted">{decodeGlossaryComment(g.comment)}</div>}
          <button type="button" onClick={() => controller.insertTarget(editor, i)}>Insert</button>
        </div>
      ))}
    </DockFrame>
  );
}
