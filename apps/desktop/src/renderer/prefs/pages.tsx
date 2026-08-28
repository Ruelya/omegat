import type { ReactElement } from "react";
import { availableLocales, t } from "../i18n";
import type { Preferences } from "../lib/types";
import { useApp } from "../store/app";

export type PrefPageProps = {
  prefs: Preferences;
  setPref: <K extends keyof Preferences>(k: K, v: Preferences[K]) => void;
  patch: (partial: Partial<Preferences>) => void;
};

function JavaKeys({
  prefs,
  setPref,
  keys,
}: {
  prefs: Preferences;
  setPref: PrefPageProps["setPref"];
  keys: string[];
}) {
  return (
    <>
      {keys.map((k) => (
        <label key={k}>
          {k}
          <input
            value={prefs.controller_keys[k] ?? ""}
            onChange={(e) => setPref("controller_keys", { ...prefs.controller_keys, [k]: e.target.value })}
          />
        </label>
      ))}
    </>
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} /> {label}
    </label>
  );
}

export function GeneralPage({ prefs, setPref }: PrefPageProps) {
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
      <Check label={t("tabAdvance")} checked={prefs.tab_advance} onChange={(v) => setPref("tab_advance", v)} />
      <Check label={t("confirmQuit")} checked={prefs.always_confirm_quit} onChange={(v) => setPref("always_confirm_quit", v)} />
      <Check label={t("firstRun")} checked={prefs.first_time_wizard_done} onChange={(v) => setPref("first_time_wizard_done", v)} />
      <JavaKeys prefs={prefs} setPref={setPref} keys={["always_confirm_quit"]} />
    </>
  );
}

export function AppearancePage({ prefs, setPref }: PrefPageProps) {
  const toggle = useApp((s) => s.toggleTheme);
  return (
    <>
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
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={["menuui_class_name", "theme_class_name", "theme_color_mode", "theme_dark_class_name"]}
      />
    </>
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
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={["dictionary_font_size", "dictionary_use_font", "project_files_use_font", "source_font", "source_font_size"]}
      />
    </>
  );
}

export function ColorsPage({ prefs, patch }: PrefPageProps) {
  const keys = [
    ["source", "color_source"],
    ["target", "color_target"],
    ["match_hit", "color_match"],
    ["glossary", "color_glossary"],
    ["nbsp", "color_nbsp"],
  ] as const;
  return (
    <>
      {keys.map(([field, label]) => (
        <label key={field}>
          {label}
          <input
            type="color"
            value={prefs.colors[field]}
            onChange={(e) => patch({ colors: { ...prefs.colors, [field]: e.target.value } })}
          />
        </label>
      ))}
    </>
  );
}

export function SavePage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("autosave")}
        <input type="number" value={prefs.autosave_seconds} onChange={(e) => setPref("autosave_seconds", Number(e.target.value))} />
      </label>
      <label>
        {t("exportTm")}
        <input value={prefs.export_tm_levels} onChange={(e) => setPref("export_tm_levels", e.target.value)} />
      </label>
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={["allow_project_extern_cmd", "auto_save_interval", "external_command", "stat_format"]}
      />
      <label>
        {t("tagValidation")}
        <select value={prefs.tag_validation} onChange={(e) => setPref("tag_validation", e.target.value)}>
          <option value="warn">warn</option>
          <option value="abort">abort</option>
        </select>
      </label>
    </>
  );
}

export function EditingPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        <input type="checkbox" checked={prefs.insert_best_match} onChange={(e) => setPref("insert_best_match", e.target.checked)} /> {t("insertBest")}
      </label>
      <Check label={t("filterUntranslated")} checked={prefs.filter_untranslated} onChange={(v) => setPref("filter_untranslated", v)} />
      <Check label={t("tabAdvance")} checked={prefs.tab_advance} onChange={(v) => setPref("tab_advance", v)} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "allowTagEditing",
          "editor_initial_segment_load_count",
          "wf_allowTransEqualToSrc",
          "wf_insertBestMatch",
          "wf_minimalSimilarity",
          "tagValidateOnLeave",
          "mark_para_delimitation_text",
          "save_auto_status",
          "save_origin",
          "wf_convertNumbers",
          "wf_explanatoryText",
          "wf_exportCurrentSegment",
          "wf_noSourceText",
          "wf_singleClickSegmentActivation",
          "wf_stopOnAlternativeTranslation",
        ]}
      />
    </>
  );
}

export function TmMatchesPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("fuzzyThreshold")}
        <input type="number" value={prefs.fuzzy_threshold} onChange={(e) => setPref("fuzzy_threshold", Number(e.target.value))} />
      </label>
      <Check label={t("matchesStem")} checked={prefs.matches_stemming_full} onChange={(v) => setPref("matches_stemming_full", v)} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "ext_tmx_fuzzy_match_threshold",
          "ext_tmx_match_template",
          "ext_tmx_show_level2",
          "keep_foreign_matches",
          "penalty_foreign_matches",
          "ext_tmx_sort_key",
          "ext_tmx_use_slash",
          "paragraph_match_from_segment_tmx",
        ]}
      />
    </>
  );
}

export function ViewPage({ prefs, patch, setPref }: PrefPageProps) {
  const m = prefs.marks;
  return (
    <>
      <Check label={t("markWhitespace")} checked={m.whitespace} onChange={(v) => patch({ marks: { ...m, whitespace: v } })} />
      <Check label={t("markNbsp")} checked={m.nbsp} onChange={(v) => patch({ marks: { ...m, nbsp: v } })} />
      <Check label={t("markBidi")} checked={m.bidi} onChange={(v) => patch({ marks: { ...m, bidi: v } })} />
      <Check label={t("markGlossary")} checked={m.glossary} onChange={(v) => patch({ marks: { ...m, glossary: v } })} />
      <Check label={t("markNoted")} checked={m.noted} onChange={(v) => patch({ marks: { ...m, noted: v } })} />
      <Check label={t("markTranslated")} checked={m.translated} onChange={(v) => patch({ marks: { ...m, translated: v } })} />
      <Check label={t("markUntranslated")} checked={m.untranslated} onChange={(v) => patch({ marks: { ...m, untranslated: v } })} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "view_option_mod_info_template",
          "view_option_source_active_bold",
          "view_option_unique_first",
          "view_option_mod_info_template_wo_date",
          "view_option_ppt_simplify",
          "view_option_source_all_bold",
          "view_option_template_active",
        ]}
      />
    </>
  );
}

export function SourceFilesViewPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("showProgress")} checked={prefs.project_files_show_translation_progress} onChange={(v) => setPref("project_files_show_translation_progress", v)} />
      <Check label={t("showOnLoad")} checked={prefs.project_files_show_on_load} onChange={(v) => setPref("project_files_show_on_load", v)} />
    </>
  );
}

export function TagProcessingPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("tagValidation")}
        <select value={prefs.tag_validation} onChange={(e) => setPref("tag_validation", e.target.value)}>
          <option value="warn">warn</option>
          <option value="abort">abort</option>
        </select>
      </label>
      <Check label={t("removeTags")} checked={prefs.remove_tags} onChange={(v) => setPref("remove_tags", v)} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "loose_tag_ordering",
          "tagValidation_customPattern",
          "tagValidation_elaborateCheck",
          "tagValidation_simpleCheck",
          "tags_valid_required",
          "tagValidation_javaMessageFormatSimplePatternCheck",
          "tagValidation_noCheck",
          "tagValidation_removePattern",
        ]}
      />
    </>
  );
}

export function SpellPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("spell")}
        <select value={prefs.spell_backend} onChange={(e) => setPref("spell_backend", e.target.value)}>
          <option value="hunspell">Hunspell</option>
          <option value="lucene">Lucene-Hunspell</option>
          <option value="morfologik">Morfologik</option>
        </select>
      </label>
      <JavaKeys prefs={prefs} setPref={setPref} keys={["allow_auto_spellchecking", "spellcheker_dir", "target_lang"]} />
    </>
  );
}

export function LanguageToolPage({ prefs, setPref }: PrefPageProps) {
  return (
    <label>
      LanguageTool URL
      <input value={prefs.languagetool_url} onChange={(e) => setPref("languagetool_url", e.target.value)} placeholder="http://localhost:8081/v2/check" />
    </label>
  );
}

export function DictionaryPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("dict")}
        <input value={prefs.dictionary_dir} onChange={(e) => setPref("dictionary_dir", e.target.value)} />
      </label>
      <Check label={t("dictFuzzy")} checked={prefs.dictionary_fuzzy_matching} onChange={(v) => setPref("dictionary_fuzzy_matching", v)} />
      <Check label={t("dictAuto")} checked={prefs.dictionary_auto_search} onChange={(v) => setPref("dictionary_auto_search", v)} />
      <JavaKeys prefs={prefs} setPref={setPref} keys={["dictionary_condensed_view"]} />
    </>
  );
}

export function GlossaryPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("glossaryStem")} checked={prefs.glossary_stem} onChange={(v) => setPref("glossary_stem", v)} />
      <Check label={t("ignoreCase")} checked={prefs.glossary_ignore_case} onChange={(v) => setPref("glossary_ignore_case", v)} />
      <Check label={t("glossaryFuzzy")} checked={prefs.glossary_not_exact_match} onChange={(v) => setPref("glossary_not_exact_match", v)} />
      <Check label={t("glossaryReplace")} checked={prefs.glossary_replace_on_insert} onChange={(v) => setPref("glossary_replace_on_insert", v)} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "glossary_merge_alternate_definitions",
          "glossary_require_similar_case",
          "glossary_sort_by_length",
          "glossary_sort_by_src_length",
          "glossary_stemming",
          "glossary_stemming_full",
          "glossary_tbx_display_context",
        ]}
      />
    </>
  );
}

const MT_ENGINES = ["google", "ibmwatson", "mymemory", "mymemory-human", "apertium", "yandex", "belazar"];

export function MachineTranslationPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("mtAutoFetch")} checked={prefs.mt_auto_fetch} onChange={(v) => setPref("mt_auto_fetch", v)} />
      {MT_ENGINES.map((eng) => (
        <label key={eng}>
          <input
            type="checkbox"
            checked={prefs.mt_enabled.includes(eng)}
            onChange={(e) => {
              const next = e.target.checked
                ? Array.from(new Set([...prefs.mt_enabled, eng]))
                : prefs.mt_enabled.filter((x) => x !== eng);
              setPref("mt_enabled", next);
            }}
          />{" "}
          {eng}
        </label>
      ))}
      <JavaKeys prefs={prefs} setPref={setPref} keys={["mt_only_untranslated"]} />
    </>
  );
}

export function AutoCompleterPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("completerAuto")} checked={prefs.completer_auto} onChange={(v) => setPref("completer_auto", v)} />
      <Check label={t("historyCompletion")} checked={prefs.history_completion} onChange={(v) => setPref("history_completion", v)} />
      <Check label={t("historyPrediction")} checked={prefs.history_prediction} onChange={(v) => setPref("history_prediction", v)} />
      <JavaKeys prefs={prefs} setPref={setPref} keys={["ac_show_suggestions_automatically", "ac_switch_views_with_lr"]} />
    </>
  );
}

export function GlossaryCompleterPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("glossaryCompleter")} checked={prefs.completer_glossary} onChange={(v) => setPref("completer_glossary", v)} />
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={[
          "ac_glossary_enabled",
          "ac_glossary_show_source",
          "ac_glossary_show_target_before_source",
          "ac_glossary_sort_alphabetically",
          "ac_glossary_sort_by_length",
        ]}
      />
    </>
  );
}

export function AutotextPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("autotext")}
        <input value={prefs.autotext} onChange={(e) => setPref("autotext", e.target.value)} placeholder="omegat=OmegaT" />
      </label>
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={["ac_autotext_enabled", "ac_autotext_sort_alphabetically", "ac_autotext_sort_by_length", "ac_autotext_sort_full_text"]}
      />
    </>
  );
}

export function CharTablePage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("charset")}
        <input value={prefs.chartable} onChange={(e) => setPref("chartable", e.target.value)} />
      </label>
      <JavaKeys
        prefs={prefs}
        setPref={setPref}
        keys={["ac_chartable_custom_char_string", "ac_chartable_enabled", "ac_chartable_unique_custom_chars", "ac_chartable_use_custom_chars"]}
      />
    </>
  );
}

export function HistoryCompleterPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("historyCompletion")} checked={prefs.history_completion} onChange={(v) => setPref("history_completion", v)} />
      <Check label={t("historyPrediction")} checked={prefs.history_prediction} onChange={(v) => setPref("history_prediction", v)} />
      <JavaKeys prefs={prefs} setPref={setPref} keys={["allow_history_completer", "history_completer_prediction_enabled"]} />
    </>
  );
}

export function TeamPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        {t("passphrase")}
        <input type="password" value={prefs.team_passphrase} onChange={(e) => setPref("team_passphrase", e.target.value)} />
      </label>
      <label>
        {t("author")}
        <input
          value={prefs.controller_keys.team_Author || ""}
          onChange={(e) => setPref("controller_keys", { ...prefs.controller_keys, team_Author: e.target.value })}
        />
      </label>
      <button type="button" onClick={() => useApp.getState().openWindow("mapping")}>
        RepositoriesMappingController
      </button>
    </>
  );
}

export function PluginsPage({ prefs, setPref }: PrefPageProps) {
  return (
    <label>
      {t("plugins")}
      <input value={prefs.plugin_dir} onChange={(e) => setPref("plugin_dir", e.target.value)} />
    </label>
  );
}

export function VersionCheckPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <Check label={t("versionCheck")} checked={prefs.version_check_enabled} onChange={(v) => setPref("version_check_enabled", v)} />
      <JavaKeys prefs={prefs} setPref={setPref} keys={["automatically_check_version"]} />
    </>
  );
}

export function SecureStorePage({ prefs, setPref }: PrefPageProps) {
  return (
    <label>
      {t("masterPassword")}
      <input type="password" value={prefs.secure_store_key} onChange={(e) => setPref("secure_store_key", e.target.value)} />
    </label>
  );
}

export function UserPassPage({ prefs, patch, setPref }: PrefPageProps) {
  return (
    <>
      {MT_ENGINES.map((eng) => (
        <label key={eng}>
          {eng} key
          <input
            type="password"
            value={prefs.mt_keys[eng] || ""}
            onChange={(e) => patch({ mt_keys: { ...prefs.mt_keys, [eng]: e.target.value } })}
          />
        </label>
      ))}
      <JavaKeys prefs={prefs} setPref={setPref} keys={["proxy_password", "proxy_user_name"]} />
    </>
  );
}

export function FiltersPage({ prefs, patch }: PrefPageProps) {
  const app = useApp();
  const filters = app.filters;
  return (
    <>
      <button
        type="button"
        data-action="open-filter-editor"
        onClick={() => {
          void app.loadFilters();
          app.openWindow("filters");
        }}
      >
        {t("filters")}
      </button>
      {filters.map((f) => (
        <div key={f.id} className="hit">
          <strong>{f.name}</strong>
          <span className="meta">{f.masks.join(", ")}</span>
          {["preserve_spaces", "file_context"].map((k) => (
            <label key={k}>
              {f.id}.{k}
              <input
                value={prefs.filter_options[f.id]?.[k] ?? ""}
                onChange={(e) =>
                  patch({
                    filter_options: {
                      ...prefs.filter_options,
                      [f.id]: { ...prefs.filter_options[f.id], [k]: e.target.value },
                    },
                  })
                }
              />
            </label>
          ))}
        </div>
      ))}
    </>
  );
}

export function SegmentationPage({ prefs, setPref }: PrefPageProps) {
  return (
    <>
      <label>
        SRX path
        <input value={prefs.srx_path} onChange={(e) => setPref("srx_path", e.target.value)} />
      </label>
      <button
        type="button"
        data-action="open-segmentation-editor"
        onClick={() => useApp.getState().openWindow("segmentation")}
      >
        SegmentationCustomizer
      </button>
    </>
  );
}

export function FinderPage({ prefs, setPref }: PrefPageProps) {
  return (
    <label>
      {t("finder")}
      <textarea rows={6} value={prefs.finder_xml} onChange={(e) => setPref("finder_xml", e.target.value)} />
      <button type="button" onClick={() => useApp.getState().openWindow("finder")}>{t("run")}</button>
    </label>
  );
}

const SHORTCUTS: [string, string][] = [
  ["project.save", "CmdOrCtrl+S"],
  ["project.compile", "CmdOrCtrl+D"],
  ["edit.insert-translation", "CmdOrCtrl+I"],
  ["edit.overwrite-translation", "CmdOrCtrl+R"],
  ["goto.untranslated", "CmdOrCtrl+U"],
  ["goto.next", "CmdOrCtrl+N"],
  ["edit.search", "CmdOrCtrl+F"],
  ["edit.replace", "CmdOrCtrl+K"],
];

export function ShortcutsPage({ prefs, patch }: PrefPageProps) {
  return (
    <table className="stats">
      <tbody>
        {SHORTCUTS.map(([id, def]) => (
          <tr key={id}>
            <td>{id}</td>
            <td>
              <input
                value={prefs.shortcuts[id] || def}
                onChange={(e) => patch({ shortcuts: { ...prefs.shortcuts, [id]: e.target.value } })}
              />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
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
  { id: "filters", title: "filters", Page: FiltersPage },
  { id: "segmentation", title: "segmentation", Page: SegmentationPage },
  { id: "finder", title: "finder", Page: FinderPage },
  { id: "shortcuts", title: "shortcuts", Page: ShortcutsPage },
];
