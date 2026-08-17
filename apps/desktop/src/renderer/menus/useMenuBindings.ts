import { useEffect } from "react";
import { useApp } from "../store/app";

export function useMenuBindings() {
  const app = useApp();
  useEffect(() => {
    const off = window.omegat?.onMenu("menu:action", (action, payload) => {
      void handle(String(action), payload);
    });
    return () => off?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app.screen, app.index, app.draft, app.marks, app.layout]);

  async function handle(action: string, _payload: unknown) {
    const a = useApp.getState();
    const e = a.entries[a.index];
    switch (action) {
      case "project.new":
        a.openWindow("wizard");
        break;
      case "project.team-new":
        a.openWindow("wizard");
        a.openWindow("team");
        break;
      case "project.open": {
        const dir = await window.omegat?.pickDir();
        if (dir) await a.open(dir);
        break;
      }
      case "project.wiki":
        a.openWindow("wiki");
        break;
      case "project.reload":
        await a.reloadProject();
        break;
      case "project.close":
        await a.closeProject();
        break;
      case "project.save":
        await a.save();
        break;
      case "project.commit-source":
        await a.teamCommit("source");
        break;
      case "project.commit-target":
        await a.teamCommit("target");
        break;
      case "project.compile":
        await a.compile();
        break;
      case "project.compile-single":
        if (e) await a.compile(e.file);
        break;
      case "project.med":
        a.openWindow("med");
        break;
      case "project.edit":
        a.openWindow("wizard");
        break;
      case "project.files":
        a.openWindow("files");
        break;
      case "project.access-root":
        if (a.props?.root) await window.omegat?.openPath(a.props.root);
        break;
      case "project.access-source":
        if (a.props?.source_dir) await window.omegat?.openPath(a.props.source_dir);
        break;
      case "project.access-target":
        if (a.props?.target_dir) await window.omegat?.openPath(a.props.target_dir);
        break;
      case "project.access-tm":
        if (a.props?.tm_dir) await window.omegat?.openPath(a.props.tm_dir);
        break;
      case "project.access-glossary":
        if (a.props?.glossary_dir) await window.omegat?.openPath(a.props.glossary_dir);
        break;
      case "project.access-dict":
        if (a.props?.dictionary_dir) await window.omegat?.openPath(a.props.dictionary_dir);
        break;
      case "project.access-export-tm":
        if (a.props?.root) await window.omegat?.openPath(a.props.root);
        break;
      case "project.access-current-source":
        if (a.props?.source_dir && e) await window.omegat?.openPath(`${a.props.source_dir}/${e.file}`);
        break;
      case "project.access-current-target":
        if (a.props?.target_dir && e) await window.omegat?.openPath(`${a.props.target_dir}/${e.file}`);
        break;
      case "project.access-writable-glossary":
        if (a.props?.glossary_file) await window.omegat?.openPath(a.props.glossary_file);
        break;
      case "edit.undo":
        a.undo();
        break;
      case "edit.redo":
        a.redo();
        break;
      case "edit.overwrite-translation":
        a.insertMatch(a.selectedMatch + 1, "overwrite");
        break;
      case "edit.insert-translation":
        a.insertMatch(a.selectedMatch + 1, "insert");
        break;
      case "edit.overwrite-mt":
        a.insertMt("overwrite");
        break;
      case "edit.overwrite-source":
        a.insertSource("overwrite");
        break;
      case "edit.insert-source":
        a.insertSource("insert");
        break;
      case "edit.tag-next":
      case "edit.tag-painter":
        a.insertTag();
        break;
      case "edit.glossary":
        a.openWindow("glossary-add");
        break;
      case "edit.search":
        a.openWindow("search");
        break;
      case "edit.replace":
        a.openWindow("replace");
        break;
      case "edit.dict":
        if (e) await a.queryDict(e.source.split(/\s+/)[0] || "");
        break;
      case "edit.case-lower":
        a.applyCase("lower");
        break;
      case "edit.case-upper":
        a.applyCase("upper");
        break;
      case "edit.case-title":
        a.applyCase("title");
        break;
      case "edit.case-sentence":
        a.applyCase("sentence");
        break;
      case "edit.case-cycle":
        a.applyCase("cycle");
        break;
      case "edit.match-1":
      case "edit.match-2":
      case "edit.match-3":
      case "edit.match-4":
      case "edit.match-5":
        a.insertMatch(Number(action.slice(-1)), "overwrite");
        break;
      case "edit.match-next":
        useApp.setState({ selectedMatch: Math.min(a.selectedMatch + 1, Math.max(0, a.matches.length - 1)) });
        break;
      case "edit.match-prev":
        useApp.setState({ selectedMatch: Math.max(0, a.selectedMatch - 1) });
        break;
      case "edit.lrm":
        a.insertChar("\u200e");
        break;
      case "edit.rlm":
        a.insertChar("\u200f");
        break;
      case "edit.lre":
        a.insertChar("\u202a");
        break;
      case "edit.rle":
        a.insertChar("\u202b");
        break;
      case "edit.pdf":
        a.insertChar("\u202c");
        break;
      case "edit.register-empty":
        await a.registerEmpty();
        break;
      case "edit.register-identical":
        await a.registerIdentical();
        break;
      case "edit.register-untranslated":
        await a.registerUntranslated();
        break;
      case "goto.next":
        await a.jump("next");
        break;
      case "goto.prev":
        await a.jump("prev");
        break;
      case "goto.untranslated":
        await a.jump("untranslated");
        break;
      case "goto.translated":
        await a.jump("translated");
        break;
      case "goto.unique":
        await a.jump("unique");
        break;
      case "goto.note-next":
        await a.jump("note");
        break;
      case "goto.auto-next":
        await a.jump("auto");
        break;
      case "goto.enforce-next":
        await a.jump("enforce");
        break;
      case "goto.history-back":
        await a.historyBack();
        break;
      case "goto.history-forward":
        await a.historyForward();
        break;
      case "goto.notes":
        useApp.setState({ focusPanel: "notes" });
        break;
      case "goto.editor":
        useApp.setState({ focusPanel: "editor" });
        break;
      case "goto.number": {
        const raw = typeof window !== "undefined" ? window.prompt("Segment number", String(a.index + 1)) : null;
        if (raw) await a.jump("number", Number(raw));
        break;
      }
      case "view.mark-translated":
        await a.toggleMark("translated");
        break;
      case "view.mark-untranslated":
        await a.toggleMark("untranslated");
        break;
      case "view.mark-paragraph":
        await a.toggleMark("paragraphStart");
        break;
      case "view.display-source":
        await a.toggleMark("displaySource");
        break;
      case "view.mark-nonunique":
        await a.toggleMark("nonUnique");
        break;
      case "view.mark-noted":
        await a.toggleMark("noted");
        break;
      case "view.mark-nbsp":
        await a.toggleMark("nbsp");
        break;
      case "view.mark-whitespace":
        await a.toggleMark("whitespace");
        break;
      case "view.mark-bidi":
        await a.toggleMark("bidi");
        break;
      case "view.mark-auto":
        await a.toggleMark("autoPopulated");
        break;
      case "view.mark-glossary":
        await a.toggleMark("glossary");
        break;
      case "view.mark-lt":
        await a.toggleMark("languageChecker");
        break;
      case "view.mark-font":
        await a.toggleMark("fontFallback");
        break;
      case "view.mark-alt":
        await a.toggleMark("alternative");
        break;
      case "view.mod-none":
        await a.setModification("none");
        break;
      case "view.mod-selected":
        await a.setModification("selected");
        break;
      case "view.mod-all":
        await a.setModification("all");
        break;
      case "view.restore-gui":
        a.restoreLayout();
        break;
      case "tools.issues":
        a.openWindow("issues");
        break;
      case "tools.issues-file":
        a.openWindow("issues");
        break;
      case "tools.stats-standard":
        a.openWindow("stats-standard");
        break;
      case "tools.stats-matches":
        a.openWindow("stats-matches");
        break;
      case "tools.stats-files":
        a.openWindow("stats-files");
        break;
      case "tools.align":
        a.openWindow("align");
        break;
      case "tools.scripts":
        a.openWindow("scripts");
        break;
      case "options.prefs":
        a.openWindow("prefs");
        break;
      case "options.filters":
        a.openWindow("filters");
        break;
      case "options.segmentation":
        a.openWindow("segmentation");
        break;
      case "options.workflow":
        a.openWindow("prefs");
        break;
      case "options.shortcuts":
        a.openWindow("shortcuts");
        break;
      case "options.mt-auto":
        useApp.setState({ mtAutoFetch: !a.mtAutoFetch });
        await a.persistMarksAndLayout();
        break;
      case "options.completer-auto":
        useApp.setState({ completerAuto: !a.completerAuto });
        await a.persistMarksAndLayout();
        break;
      case "options.history-completion":
        useApp.setState({ historyCompletion: !a.historyCompletion });
        await a.persistMarksAndLayout();
        break;
      case "options.history-prediction":
        useApp.setState({ historyPrediction: !a.historyPrediction });
        await a.persistMarksAndLayout();
        break;
      case "options.glossary-fuzzy":
        await a.patchPrefs({}, { glossary_not_exact_match: a.prefs?.extra.glossary_not_exact_match === "true" ? "false" : "true" });
        break;
      case "options.dict-fuzzy":
        await a.patchPrefs({}, { dictionary_fuzzy_matching: a.prefs?.extra.dictionary_fuzzy_matching === "true" ? "false" : "true" });
        break;
      case "options.config-dir":
        if (a.prefs?.config_dir) await window.omegat?.openPath(a.prefs.config_dir);
        break;
      case "help.about":
        a.openWindow("about");
        break;
      case "help.license":
        a.openWindow("license");
        break;
      case "help.log":
        a.openWindow("log");
        break;
      case "help.tip":
        a.openWindow("tip");
        break;
      case "help.manual":
        await (window.omegat as { openManual?: () => Promise<void> })?.openManual?.();
        break;
      case "help.updates":
        await window.omegat?.openExternal("https://omegat.org/download");
        break;
      default:
        break;
    }
  }
}
