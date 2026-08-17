import type { ReactElement } from "react";
import { availableLocales, t } from "../i18n";
import type { Preferences } from "../lib/types";
import { useApp } from "../store/app";

export type PrefPageProps = {
  prefs: Preferences;
  extra: Record<string, string>;
  setPref: <K extends keyof Preferences>(k: K, v: Preferences[K]) => void;
  setExtra: (k: string, v: string) => void;
};

function Check({
  label,
  k,
  extra,
  setExtra,
  def = false,
}: {
  label: string;
  k: string;
  extra: Record<string, string>;
  setExtra: (k: string, v: string) => void;
  def?: boolean;
}) {
  const on = extra[k] == null ? def : extra[k] === "true";
  return (
    <label>
      <input type="checkbox" checked={on} onChange={(e) => setExtra(k, String(e.target.checked))} /> {label}
    </label>
  );
}

export function GeneralPage({ prefs, extra, setPref, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        {t("uiLanguage")}
        <select value={prefs.locale} onChange={(e) => setPref("locale", e.target.value)}>
          {availableLocales().map((code) => (
            <option key={code} value={code}>{code}</option>
          ))}
        </select>
      </label>
      <Check label={t("tabAdvance")} k="tab_advance" extra={extra} setExtra={setExtra} />
      <Check label={t("confirmQuit")} k="always_confirm_quit" extra={extra} setExtra={setExtra} />
      <Check label={t("firstRun")} k="first_time_wizard_done" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function AppearancePage({ prefs, setPref }: PrefPageProps) {
  const toggle = useApp((s) => s.toggleTheme);
  return (
    <label>
      {t("appearance")}
      <select
        value={prefs.theme}
        onChange={() => {
          toggle();
          setPref("theme", prefs.theme === "light" ? "dark" : "light");
        }}
      >
        <option value="light">light</option>
        <option value="dark">dark</option>
      </select>
    </label>
  );
}

export function FontsPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("fontUi")}
        <input value={prefs.font_ui} onChange={(e) => setPref("font_ui", e.target.value)} />
      </label>
      <label>
        {t("fontEditor")}
        <input value={prefs.font_editor} onChange={(e) => setPref("font_editor", e.target.value)} />
      </label>
    </>
  );
}

export function ColorsPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      {["color_source", "color_target", "color_match", "color_glossary", "color_nbsp"].map((k) => (
        <label key={k}>
          {k}
          <input type="color" value={extra[k] || "#9b2c1a"} onChange={(e) => setExtra(k, e.target.value)} />
        </label>
      ))}
    </>
  );
}

export function SavePage({ prefs, extra, setPref, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        {t("autosave")}
        <input type="number" value={prefs.autosave_seconds} onChange={(e) => setPref("autosave_seconds", Number(e.target.value))} />
      </label>
      <label>
        {t("exportTm")}
        <input value={extra.export_tm_levels || "omegat level1 level2"} onChange={(e) => setExtra("export_tm_levels", e.target.value)} />
      </label>
      <label>
        {t("tagValidation")}
        <select value={extra.tag_validation || "warn"} onChange={(e) => setExtra("tag_validation", e.target.value)}>
          <option value="warn">warn</option>
          <option value="abort">abort</option>
        </select>
      </label>
    </>
  );
}

export function EditingPage({ prefs, extra, setPref, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        <input type="checkbox" checked={prefs.insert_best_match} onChange={(e) => setPref("insert_best_match", e.target.checked)} /> {t("insertBest")}
      </label>
      <Check label={t("filterUntranslated")} k="filter_untranslated" extra={extra} setExtra={setExtra} />
      <Check label={t("tabAdvance")} k="tab_advance" extra={extra} setExtra={setExtra} />
    </>
  );
}

export function TmMatchesPage({ prefs, extra, setPref, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        {t("fuzzyThreshold")}
        <input type="number" value={prefs.fuzzy_threshold} onChange={(e) => setPref("fuzzy_threshold", Number(e.target.value))} />
      </label>
      <Check label={t("matchesStem")} k="matches_stemming_full" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function ViewPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("markWhitespace")} k="mark_whitespace" extra={extra} setExtra={setExtra} />
      <Check label={t("markNbsp")} k="mark_nbsp" extra={extra} setExtra={setExtra} />
      <Check label={t("markBidi")} k="mark_bidi" extra={extra} setExtra={setExtra} />
      <Check label={t("markGlossary")} k="mark_glossary_matches" extra={extra} setExtra={setExtra} def />
      <Check label={t("markNoted")} k="mark_noted_segments" extra={extra} setExtra={setExtra} def />
      <Check label={t("markTranslated")} k="mark_translated" extra={extra} setExtra={setExtra} def />
      <Check label={t("markUntranslated")} k="mark_untranslated" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function SourceFilesViewPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("showProgress")} k="project_files_show_translation_progress" extra={extra} setExtra={setExtra} def />
      <Check label={t("showOnLoad")} k="project_files_show_on_load" extra={extra} setExtra={setExtra} />
    </>
  );
}

export function TagProcessingPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        {t("tagValidation")}
        <select value={extra.tag_validation || "warn"} onChange={(e) => setExtra("tag_validation", e.target.value)}>
          <option value="warn">warn</option>
          <option value="abort">abort</option>
        </select>
      </label>
      <Check label={t("removeTags")} k="remove_tags" extra={extra} setExtra={setExtra} />
    </>
  );
}

export function SpellPage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("spell")}
      <select value={extra.spell_backend || "hunspell"} onChange={(e) => setExtra("spell_backend", e.target.value)}>
        <option value="hunspell">Hunspell</option>
        <option value="lucene">Lucene-Hunspell</option>
        <option value="morfologik">Morfologik</option>
      </select>
    </label>
  );
}

export function LanguageToolPage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      LanguageTool URL
      <input value={extra.languagetool_url || ""} onChange={(e) => setExtra("languagetool_url", e.target.value)} placeholder="http://localhost:8081/v2/check" />
    </label>
  );
}

export function DictionaryPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <label>
        {t("dict")}
        <input value={extra.dictionary_dir || "dictionary"} onChange={(e) => setExtra("dictionary_dir", e.target.value)} />
      </label>
      <Check label={t("dictFuzzy")} k="dictionary_fuzzy_matching" extra={extra} setExtra={setExtra} />
      <Check label={t("dictAuto")} k="dictionary_auto_search" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function GlossaryPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("glossaryStem")} k="glossary_stem" extra={extra} setExtra={setExtra} def />
      <Check label={t("ignoreCase")} k="glossary_ignore_case" extra={extra} setExtra={setExtra} def />
      <Check label={t("glossaryFuzzy")} k="glossary_not_exact_match" extra={extra} setExtra={setExtra} />
      <Check label={t("glossaryReplace")} k="glossary_replace_on_insert" extra={extra} setExtra={setExtra} />
    </>
  );
}

const MT_ENGINES = ["google", "ibmwatson", "mymemory", "mymemory-human", "apertium", "yandex", "belazar"];

export function MachineTranslationPage({ prefs, extra, setPref, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("mtAutoFetch")} k="mt_auto_fetch" extra={extra} setExtra={setExtra} />
      {MT_ENGINES.map((eng) => (
        <label key={eng}>
          <input
            type="checkbox"
            checked={prefs.mt_enabled.includes(eng) || extra[`mt.${eng}`] === "true"}
            onChange={(e) => {
              setExtra(`mt.${eng}`, String(e.target.checked));
              const next = e.target.checked
                ? Array.from(new Set([...prefs.mt_enabled, eng]))
                : prefs.mt_enabled.filter((x) => x !== eng);
              setPref("mt_enabled", next);
            }}
          />{" "}
          {eng}
        </label>
      ))}
    </>
  );
}

export function AutoCompleterPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("completerAuto")} k="completer_auto" extra={extra} setExtra={setExtra} def />
      <Check label={t("historyCompletion")} k="history_completion" extra={extra} setExtra={setExtra} def />
      <Check label={t("historyPrediction")} k="history_prediction" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function GlossaryCompleterPage({ extra, setExtra }: PrefPageProps) {
  return <Check label={t("glossaryCompleter")} k="completer_glossary" extra={extra} setExtra={setExtra} def />;
}

export function AutotextPage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("autotext")}
      <input value={extra.autotext || ""} onChange={(e) => setExtra("autotext", e.target.value)} placeholder="omegat=OmegaT" />
    </label>
  );
}

export function CharTablePage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("charset")}
      <input value={extra.chartable || "©®™…—–«»"} onChange={(e) => setExtra("chartable", e.target.value)} />
    </label>
  );
}

export function HistoryCompleterPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      <Check label={t("historyCompletion")} k="history_completion" extra={extra} setExtra={setExtra} def />
      <Check label={t("historyPrediction")} k="history_prediction" extra={extra} setExtra={setExtra} def />
    </>
  );
}

export function TeamPage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("passphrase")}
      <input type="password" value={extra.team_passphrase || ""} onChange={(e) => setExtra("team_passphrase", e.target.value)} />
    </label>
  );
}

export function PluginsPage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("plugins")}
      <input value={extra.plugin_dir || "plugins"} onChange={(e) => setExtra("plugin_dir", e.target.value)} />
    </label>
  );
}

export function VersionCheckPage({ extra, setExtra }: PrefPageProps) {
  return <Check label={t("versionCheck")} k="version_check_enabled" extra={extra} setExtra={setExtra} def />;
}

export function SecureStorePage({ extra, setExtra }: PrefPageProps) {
  return (
    <label>
      {t("masterPassword")}
      <input type="password" value={extra.secure_store_key || ""} onChange={(e) => setExtra("secure_store_key", e.target.value)} />
    </label>
  );
}

export function UserPassPage({ extra, setExtra }: PrefPageProps) {
  return (
    <>
      {MT_ENGINES.map((eng) => (
        <label key={eng}>
          {eng} key
          <input type="password" value={extra[`mt.${eng}.key`] || ""} onChange={(e) => setExtra(`mt.${eng}.key`, e.target.value)} />
        </label>
      ))}
    </>
  );
}

export const PREF_PAGES: { id: string; title: string; Page: (p: PrefPageProps) => ReactElement }[] = [
  { id: "general", title: "general", Page: GeneralPage },
  { id: "appearance", title: "appearance", Page: AppearancePage },
  { id: "fonts", title: "fonts", Page: FontsPage },
  { id: "colors", title: "colors", Page: ColorsPage },
  { id: "save", title: "save", Page: SavePage },
  { id: "editing", title: "editing", Page: EditingPage },
  { id: "matches", title: "matches", Page: TmMatchesPage },
  { id: "view", title: "view", Page: ViewPage },
  { id: "source-files", title: "sourceFilesView", Page: SourceFilesViewPage },
  { id: "tags", title: "tagProcessing", Page: TagProcessingPage },
  { id: "spell", title: "spell", Page: SpellPage },
  { id: "languagetool", title: "languagetool", Page: LanguageToolPage },
  { id: "dict", title: "dict", Page: DictionaryPage },
  { id: "glossary", title: "glossary", Page: GlossaryPage },
  { id: "mt", title: "mt", Page: MachineTranslationPage },
  { id: "completer", title: "completer", Page: AutoCompleterPage },
  { id: "glossary-completer", title: "glossaryCompleter", Page: GlossaryCompleterPage },
  { id: "autotext", title: "autotext", Page: AutotextPage },
  { id: "chartable", title: "charset", Page: CharTablePage },
  { id: "history-completer", title: "historyCompleter", Page: HistoryCompleterPage },
  { id: "team", title: "team", Page: TeamPage },
  { id: "plugins", title: "plugins", Page: PluginsPage },
  { id: "version-check", title: "versionCheck", Page: VersionCheckPage },
  { id: "secure-store", title: "secureStore", Page: SecureStorePage },
  { id: "user-pass", title: "userPass", Page: UserPassPage },
];
