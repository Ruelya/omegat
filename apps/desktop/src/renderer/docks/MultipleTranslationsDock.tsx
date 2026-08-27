import { useState } from "react";
import { t } from "../i18n";
import { sameCompleteEntryKey } from "../editor/EditorController";
import {
  MultipleTranslationsController,
  type MultipleTranslationTarget,
} from "../lib/dock-controllers";
import { useApp } from "../store/app";
import { DockFrame } from "./DockFrame";

export function MultipleTranslationsDock() {
  const entries = useApp((s) => s.entries);
  const index = useApp((s) => s.index);
  const e = entries[index];
  const draft = useApp((s) => s.draft);
  const setDraft = useApp((s) => s.setDraft);
  const commitCurrent = useApp((s) => s.commitCurrent);
  const select = useApp((s) => s.select);
  const [selectedRow, setSelectedRow] = useState(0);
  const controller = new MultipleTranslationsController(entries, index);
  const editor: MultipleTranslationTarget = {
    getCurrentTranslation: () => draft,
    replaceEditText: setDraft,
    insertText: (text) => setDraft(draft + text),
    commitTranslationVariant: async (defaultTranslation) => {
      await commitCurrent({ default_translation: defaultTranslation });
    },
    gotoEntry: async (source, key) => {
      const target = entries.findIndex((entry) =>
        entry.source === source
        && sameCompleteEntryKey(entry.key, key)
      );
      if (target >= 0) await select(target);
      return target >= 0;
    },
  };
  const activeRow = controller.rows[selectedRow];
  return (
    <DockFrame
      title={t("multiple")}
      menu={[
        {
          id: "replace",
          label: t("replace"),
          disabled: activeRow === undefined,
          action: () => controller.replace(editor, selectedRow),
        },
        {
          id: "default",
          label: t("defaultTranslation"),
          disabled: activeRow === undefined || activeRow.isDefault,
          action: () => controller.makeDefault(editor, selectedRow),
        },
        {
          id: "goto",
          label: "Go to segment",
          separatorBefore: true,
          disabled: activeRow === undefined,
          action: () => controller.goto(editor, selectedRow),
        },
      ]}
    >
      <div className="empty-state">{e?.default_translation ? t("defaultTranslation") : t("alternateTranslation")}</div>
      {controller.rows.map((row, rowIndex) => (
        <div
          key={JSON.stringify(row.key)}
          className={`hit ${rowIndex === selectedRow ? "active" : ""}`}
          onClick={() => setSelectedRow(rowIndex)}
        >
          #{row.index + 1} {row.file}{row.id ? `/${row.id}` : ""} — {row.translation || "—"}
          {row.previous !== null && row.next !== null && (
            <div className="muted">({row.previous} &lt;...&gt; {row.next})</div>
          )}
          <div>
            <button type="button" onClick={() => controller.replace(editor, rowIndex)}>{t("replace")}</button>
            {!row.isDefault && (
              <button type="button" onClick={() => controller.makeDefault(editor, rowIndex)}>
                {t("defaultTranslation")}
              </button>
            )}
            <button type="button" onClick={() => controller.goto(editor, rowIndex)}>Go to</button>
          </div>
        </div>
      ))}
    </DockFrame>
  );
}
