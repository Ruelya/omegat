import { t } from "../i18n";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MatchesDock() {
  const matches = useApp((s) => s.matches);
  const selected = useApp((s) => s.selectedMatch);
  const insertMatch = useApp((s) => s.insertMatch);
  return (
    <DockFrame title={t("matches")}>
      {matches.map((m, i) => (
        <div
          key={`${m.comes_from}-${i}`}
          className={`hit ${i === selected ? "active" : ""}`}
          onClick={() => insertMatch(i + 1, "overwrite")}
        >
          <div className="score">
            {m.score}% {m.comes_from}
            {m.adjusted_score != null && m.adjusted_score !== m.score ? ` · adj ${m.adjusted_score}` : ""}
          </div>
          <div className="muted">{m.source}</div>
          <div>{m.translation}</div>
        </div>
      ))}
    </DockFrame>
  );
}
