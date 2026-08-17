import { useEffect } from "react";
import { Books, Gear, Moon, Sun } from "@phosphor-icons/react";
import { WorkspaceDocks } from "./docks/DockLayout";
import { t } from "./i18n";
import { useMenuBindings } from "./menus/useMenuBindings";
import { PrefsWindow } from "./prefs/PrefsWindow";
import { SearchWindow } from "./search/SearchWindow";
import { useApp } from "./store/app";
import { Welcome } from "./welcome/Welcome";
import { AboutWindow, ChangesWindow, LicenseWindow, LogWindow } from "./windows/AboutLicenseLog";
import { FilesWindow, IssuesWindow } from "./windows/FilesIssues";
import { StatsWindow } from "./windows/StatsWindows";
import { TipOfDay } from "./windows/TipOfDay";
import {
  AlignWindow,
  ConvertWindow,
  FiltersWindow,
  GlossaryAddWindow,
  MedWindow,
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
    void app.loadVersion();
    void app.loadPrefs();
    if (app.firstRun) app.openWindow("tip");
  }, []);

  useEffect(() => {
    const seconds = app.prefs?.autosave_seconds ?? 0;
    if (!seconds || app.screen !== "workspace") return;
    const id = window.setInterval(() => {
      void useApp.getState().save();
    }, seconds * 1000);
    return () => window.clearInterval(id);
  }, [app.prefs?.autosave_seconds, app.screen]);

  const w = app.windows;
  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          Omega<span>T</span>
        </div>
        <span className="muted">{app.version && `v${app.version}`}</span>
        <div style={{ flex: 1 }} />
        {app.screen === "workspace" && (
          <>
            <button type="button" onClick={() => void app.save()}>{t("save")}</button>
            <button type="button" className="primary" onClick={() => void app.compile()}>{t("compile")}</button>
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
          <span>{app.props?.root}</span>
        </footer>
      )}
      {w.wizard && <Wizard />}
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
