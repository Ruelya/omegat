import { t } from "../i18n";
import { decodeGlossaryComment, renderGlossaryText } from "../lib/dock-models";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function GlossaryDock() {
  const glossary = useApp((s) => s.glossary);
  const setDraft = useApp((s) => s.setDraft);
  const draft = useApp((s) => s.draft);
  const plainText = renderGlossaryText(glossary);
  return (
    <DockFrame title={t("glossary")}>
      <span className="sr-only" aria-label={plainText} />
      {glossary.map((g, i) => (
        <div key={`${g.source}-${i}`} className="hit" onClick={() => setDraft(draft + g.target)}>
          <b>{g.source}</b> → {g.target}
          {g.comment && <div className="muted">{decodeGlossaryComment(g.comment)}</div>}
        </div>
      ))}
    </DockFrame>
  );
}
