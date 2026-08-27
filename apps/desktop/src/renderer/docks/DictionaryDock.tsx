import { useState } from "react";
import { t } from "../i18n";
import { DictionaryController } from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function DictionaryDock() {
  const dict = useApp((s) => s.dict);
  const queryDict = useApp((s) => s.queryDict);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(-1);
  const controller = new DictionaryController(dict);
  return (
    <DockFrame title={t("dict")}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (query.trim()) void queryDict(query.trim());
        }}
      >
        <input value={query} onChange={(event) => setQuery(event.target.value)} aria-label={t("search")} />
        <button type="submit">{t("search")}</button>
      </form>
      {controller.entries.map((d, i) => (
        <div
          key={`${d.word}-${d.source}-${i}`}
          className={`hit ${i === selected ? "active" : ""}`}
          onClick={() => setSelected(controller.focusWord(d.word))}
        >
          <b>{d.word}</b> {d.definition}
          <div className="muted">{d.source}</div>
        </div>
      ))}
    </DockFrame>
  );
}
