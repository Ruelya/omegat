import { t } from "../i18n";
import type { WindowId } from "../lib/types";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function StatsWindow({ kind }: { kind: "standard" | "matches" | "files" }) {
  const stats = useApp((s) => s.stats);
  const id = (`stats-${kind === "standard" ? "standard" : kind === "matches" ? "matches" : "files"}`) as WindowId;
  return (
    <Modal id={id} title={t(`stats-${kind}`)} wide>
      {!stats && <p className="muted">—</p>}
      {stats && kind === "standard" && (
        <table className="stats">
          <tbody>
            <tr><td>{t("files")}</td><td>{stats.files}</td></tr>
            <tr><td>{t("segments")}</td><td>{stats.segments}</td></tr>
            <tr><td>{t("translated")}</td><td>{stats.translated}</td></tr>
            <tr><td>{t("unique")}</td><td>{stats.unique_segments}</td></tr>
            <tr><td>{t("sourceWords")}</td><td>{stats.source_words}</td></tr>
            <tr><td>{t("targetWords")}</td><td>{stats.target_words}</td></tr>
          </tbody>
        </table>
      )}
      {stats && kind === "matches" && stats.match_bins && (
        <table className="stats">
          <tbody>
            <tr><td>exact</td><td>{stats.match_bins.exact}</td></tr>
            <tr><td>95</td><td>{stats.match_bins.fuzzy_95}</td></tr>
            <tr><td>85</td><td>{stats.match_bins.fuzzy_85}</td></tr>
            <tr><td>75</td><td>{stats.match_bins.fuzzy_75}</td></tr>
            <tr><td>50</td><td>{stats.match_bins.fuzzy_50}</td></tr>
            <tr><td>none</td><td>{stats.match_bins.none}</td></tr>
          </tbody>
        </table>
      )}
      {stats && kind === "files" && (
        <table className="stats">
          <thead><tr><th>file</th><th>total</th><th>remaining</th></tr></thead>
          <tbody>
            {(stats.file_stats ?? []).map((f) => (
              <tr key={f.filename}>
                <td>{f.filename}</td>
                <td>{f.total.segments}</td>
                <td>{f.remaining.segments}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <button type="button" onClick={() => useApp.getState().openWindow(id, false)}>{t("cancel")}</button>
    </Modal>
  );
}
