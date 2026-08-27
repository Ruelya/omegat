import { useEffect } from "react";
import { Books, Gear, Moon, Sun } from "@phosphor-icons/react";
import { WorkspaceDocks } from "./docks/DockLayout";
import { t } from "./i18n";
import { useMenuBindings } from "./menus/useMenuBindings";
import { PrefsWindow } from "./prefs/PrefsWindow";
import { SearchWindow } from "./search/SearchWindow";
import {
  connectExternalProjectEvents,
  connectRpcOperationEvents,
  useApp,
} from "./store/app";
import { Welcome } from "./welcome/Welcome";
import { AboutWindow, ChangesWindow, LicenseWindow, LogWindow } from "./windows/AboutLicenseLog";
import { FilesWindow, IssuesWindow } from "./windows/FilesIssues";
import { StatsWindow } from "./windows/StatsWindows";
import { TipOfDay } from "./windows/TipOfDay";
import {
  AlignWindow,
  ConvertWindow,
  FiltersWindow,
  FinderWindow,
  GlossaryAddWindow,
  MappingWindow,
  MedWindow,
  ProjectEditWindow,
  ScriptsWindow,
  SegmentationWindow,
  ShortcutsWindow,
  TeamWindow,
  WikiWindow,
} from "./windows/ToolsWindows";
import { Wizard } from "./windows/Wizard";

export function App() {
  const app = useApp();
  useMenuBindings();

  useEffect(() => {
    void (async () => {
      await Promise.all([app.loadVersion(), app.loadPrefs()]);
      const startup = await window.omegat?.startup?.();
      if (startup?.project) {
        await app.open(startup.project);
      }
      if (useApp.getState().firstRun) app.openWindow("tip");
    })();
  }, []);

  useEffect(() => connectExternalProjectEvents(), []);
  useEffect(() => connectRpcOperationEvents(), []);

  useEffect(() => {
    const root = app.props?.root;
    const generation = app.projectEvent.projectGeneration;
    if (root) {
      void window.omegat?.watchProject?.(root, generation);
    } else {
      void window.omegat?.unwatchProject?.();
    }
    return () => {
      if (root) void window.omegat?.unwatchProject?.();
    };
  }, [app.props?.root, app.projectEvent.projectGeneration]);

  useEffect(() => {
    const seconds = app.prefs?.autosave_seconds ?? 0;
    if (!seconds || app.screen !== "workspace") return;
    const id = window.setInterval(() => {
      void useApp.getState().save();
    }, seconds * 1000);
    return () => window.clearInterval(id);
  }, [app.prefs?.autosave_seconds, app.screen]);

  const w = app.windows;
  const operationActive = app.longOperation
    && (
      app.longOperation.phase === "started"
      || app.longOperation.phase === "progress"
      || app.longOperation.phase === "cancelling"
    );
  const operationText = app.longOperation
    ? `${app.longOperation.kind}: ${app.longOperation.stage ?? app.longOperation.phase}`
    : "";
  return (
    <div
      className="app"
      data-project-event={app.projectEvent.kind}
      data-project-generation={app.projectEvent.projectGeneration}
      data-entry-generation={app.projectEvent.entryGeneration}
      data-project-id={app.projectEvent.projectId ?? ""}
      data-operation={app.longOperation?.kind ?? ""}
      data-operation-phase={app.longOperation?.phase ?? ""}
      data-operation-stage={app.longOperation?.stage ?? ""}
      data-operation-request-id={app.longOperation?.requestId ?? ""}
    >
      <header className="topbar">
        <div className="brand">
          Omega<span>T</span>
        </div>
        <span className="muted">{app.version && `v${app.version}`}</span>
        <div style={{ flex: 1 }} />
        {app.screen === "workspace" && (
          <>
            <button type="button" onClick={() => void app.save()}>{t("save")}</button>
            <button
              type="button"
              className="primary"
              data-operation-action="compile"
              disabled={Boolean(operationActive)}
              onClick={() => void app.compile()}
            >
              {t("compile")}
            </button>
            {operationActive && (
              <button
                type="button"
                data-operation-action="cancel"
                disabled={app.longOperation?.phase === "cancelling"}
                onClick={() => void app.cancelLongOperation()}
              >
                {app.longOperation?.phase === "cancelling" ? "Cancelling…" : t("cancel")}
              </button>
            )}
            <button type="button" onClick={() => app.openWindow("search")}>{t("search")}</button>
            <button type="button" onClick={() => app.openWindow("files")}>{t("files")}</button>
            <button type="button" onClick={() => app.openWindow("issues")}>{t("issues")}</button>
          </>
        )}
        <button type="button" onClick={() => app.openWindow("prefs")} aria-label={t("prefs")}>
          <Gear size={16} />
        </button>
        <button type="button" onClick={() => app.openWindow("about")} aria-label={t("about")}>
          <Books size={16} />
        </button>
        <button type="button" onClick={app.toggleTheme} aria-label="theme">
          {app.theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
        </button>
      </header>
      {app.screen === "welcome" ? <Welcome /> : <WorkspaceDocks />}
      {app.screen === "workspace" && (
        <footer className="status">
          <span>{app.stats ? `${app.stats.translated}/${app.stats.segments}` : ""}</span>
          <span>{app.props ? `${app.props.source_lang} → ${app.props.target_lang}` : ""}</span>
          <span>{app.status}</span>
          <span
            role="status"
            aria-live="polite"
            data-operation-status
          >
            {operationText}
          </span>
          <span>{app.props?.root}</span>
        </footer>
      )}
      {w.wizard && <Wizard />}
      {w["project-edit"] && <ProjectEditWindow />}
      {w.finder && <FinderWindow />}
      {w.search && <SearchWindow mode="search" />}
      {w.replace && <SearchWindow mode="replace" />}
      {w.prefs && <PrefsWindow />}
      {w.about && <AboutWindow />}
      {w.license && <LicenseWindow />}
      {w.log && <LogWindow />}
      {w.changes && <ChangesWindow />}
      {w.tip && <TipOfDay />}
      {w.align && <AlignWindow />}
      {w.team && <TeamWindow />}
      {w.mapping && <MappingWindow />}
      {w.files && <FilesWindow />}
      {w.issues && <IssuesWindow />}
      {w["stats-standard"] && <StatsWindow kind="standard" />}
      {w["stats-matches"] && <StatsWindow kind="matches" />}
      {w["stats-files"] && <StatsWindow kind="files" />}
      {w.filters && <FiltersWindow />}
      {w.segmentation && <SegmentationWindow />}
      {w.shortcuts && <ShortcutsWindow />}
      {w.wiki && <WikiWindow />}
      {w.med && <MedWindow />}
      {w.convert && <ConvertWindow />}
      {w.scripts && <ScriptsWindow />}
      {w["glossary-add"] && <GlossaryAddWindow />}
      {app.error && <div className="status">{app.error}</div>}
    </div>
  );
}
