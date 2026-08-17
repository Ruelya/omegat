import { t } from "../i18n";
import { useApp } from "../store/app";

export function SearchWindow({ mode }: { mode: "search" | "replace" }) {
  const form = useApp((s) => s.searchForm);
  const set = useApp((s) => s.setSearchForm);
  const hits = useApp((s) => s.searchHits);
  const run = useApp((s) => s.runSearch);
  const replaceAll = useApp((s) => s.replaceAll);
  const select = useApp((s) => s.select);
  const close = () => useApp.getState().openWindow(mode === "replace" ? "replace" : "search", false);
  return (
    <div className="modal-bg" onClick={close}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h2>{mode === "replace" ? t("replace") : t("search")}</h2>
        <div className="form">
          <label>
            {t("search")}
            <input
              autoFocus
              value={form.query}
              onChange={(e) => {
                set({ query: e.target.value });
                if (e.target.value) void run(false);
              }}
            />
          </label>
          {mode === "replace" && (
            <label>
              {t("replace")}
              <input value={form.replace} onChange={(e) => set({ replace: e.target.value })} />
            </label>
          )}
          <fieldset className="checks">
            <legend>{t("searchType")}</legend>
            {(["exact", "keyword", "regex"] as const).map((ty) => (
              <label key={ty}>
                <input
                  type="radio"
                  name="stype"
                  checked={form.searchType === ty}
                  onChange={() => set({ searchType: ty })}
                />{" "}
                {t(ty)}
              </label>
            ))}
          </fieldset>
          <fieldset className="checks">
            <legend>{t("searchIn")}</legend>
            <label><input type="checkbox" checked={form.source} onChange={(e) => set({ source: e.target.checked })} /> {t("source")}</label>
            <label><input type="checkbox" checked={form.translation} onChange={(e) => set({ translation: e.target.checked })} /> {t("target")}</label>
            <label><input type="checkbox" checked={form.notes} onChange={(e) => set({ notes: e.target.checked })} /> {t("notes")}</label>
            <label><input type="checkbox" checked={form.comments} onChange={(e) => set({ comments: e.target.checked })} /> {t("comments")}</label>
          </fieldset>
          <fieldset className="checks">
            <legend>{t("options")}</legend>
            <label><input type="checkbox" checked={form.caseSensitive} onChange={(e) => set({ caseSensitive: e.target.checked })} /> {t("caseSensitive")}</label>
            <label><input type="checkbox" checked={form.wholeWord} onChange={(e) => set({ wholeWord: e.target.checked })} /> {t("wholeWord")}</label>
            <label><input type="checkbox" checked={form.untranslated} onChange={(e) => set({ untranslated: e.target.checked })} /> {t("untranslatedOnly")}</label>
          </fieldset>
          <label>
            {t("author")}
            <input value={form.author} onChange={(e) => set({ author: e.target.value })} />
          </label>
          <div className="row-2">
            <label>
              {t("dateFrom")}
              <input value={form.dateFrom} onChange={(e) => set({ dateFrom: e.target.value })} placeholder="20200101T000000Z" />
            </label>
            <label>
              {t("dateTo")}
              <input value={form.dateTo} onChange={(e) => set({ dateTo: e.target.value })} />
            </label>
          </div>
          <div className="btn-row">
            <button type="button" className="primary" onClick={() => void run(false)}>{t("search")}</button>
            {mode === "replace" && (
              <>
                <button type="button" onClick={() => void run(true)}>{t("replacePreview")}</button>
                <button type="button" onClick={() => void replaceAll()}>{t("replace")}</button>
              </>
            )}
            <button type="button" onClick={close}>{t("cancel")}</button>
          </div>
        </div>
        <div className="list">
          {hits.map((h, i) => (
            <div
              key={`${h.index}-${h.field}-${i}`}
              className="hit"
              onClick={() => {
                void select(h.index);
                close();
              }}
            >
              <span className="meta">#{h.index + 1} {h.field}</span> {h.text}
              {h.preview != null && <div className="muted">→ {h.preview}</div>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
