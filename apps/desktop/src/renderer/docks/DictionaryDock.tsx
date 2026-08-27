import { useState } from "react";
import { t } from "../i18n";
import {
  DictionaryController,
  DockNotificationController,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function DictionaryDock() {
  const dict = useApp((s) => s.dict);
  const queryDict = useApp((s) => s.queryDict);
  const openWindow = useApp((s) => s.openWindow);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(-1);
  const [notifyHits, setNotifyHits] = useState(true);
  const [notifyMisses, setNotifyMisses] = useState(false);
  const controller = new DictionaryController(dict);
  const notifications = new DockNotificationController(notifyHits, notifyMisses);
  return (
    <DockFrame
      title={t("dict")}
      notification={query ? notifications.signal(controller.entries.length) : null}
      menu={[
        {
          id: "notify-hits",
          label: "Notify on dictionary hits",
          checked: notifyHits,
          action: () => setNotifyHits((enabled) => !enabled),
        },
        {
          id: "notify-misses",
          label: "Notify when no dictionary entry is found",
          checked: notifyMisses,
          action: () => setNotifyMisses((enabled) => !enabled),
        },
        {
          id: "preferences",
          label: "Dictionary preferences",
          separatorBefore: true,
          action: () => openWindow("prefs"),
        },
      ]}
    >
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
