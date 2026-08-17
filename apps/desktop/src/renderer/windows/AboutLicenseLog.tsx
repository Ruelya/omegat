import { t } from "../i18n";
import { useApp } from "../store/app";
import { Modal } from "./Modal";

export function AboutWindow() {
  const version = useApp((s) => s.version);
  return (
    <Modal id="about" title={`OmegaT ${version}`}>
      <p>GNU GPL v3+. React + Vite + Electron / Rust sidecar. No embedded JVM.</p>
      <p className="muted">https://omegat.org</p>
      <button type="button" onClick={() => useApp.getState().openWindow("about", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function LicenseWindow() {
  return (
    <Modal id="license" title={t("license")} wide>
      <pre className="license">
{`OmegaT is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

See the LICENSE file in the project root and
https://www.gnu.org/licenses/gpl-3.0.html`}
      </pre>
      <button type="button" onClick={() => useApp.getState().openWindow("license", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function ChangesWindow() {
  return (
    <Modal id="changes" title={t("lastChanges")} wide>
      <pre className="log">{`OmegaT 6.2 rewrite
- Document-model editor (tags are atomic)
- Typed preferences (28 controllers)
- 120 Java menu actions + script slots 1–12
See docs/rewrite/STATUS.md`}</pre>
      <button type="button" onClick={() => useApp.getState().openWindow("changes", false)}>{t("cancel")}</button>
    </Modal>
  );
}

export function LogWindow() {
  const log = useApp((s) => s.log);
  return (
    <Modal id="log" title={t("log")} wide>
      <pre className="log">{log.join("\n") || "—"}</pre>
      <button type="button" onClick={() => useApp.getState().openWindow("log", false)}>{t("cancel")}</button>
    </Modal>
  );
}
