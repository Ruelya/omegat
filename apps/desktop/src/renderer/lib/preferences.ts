import { decorateText, marksFromPrefs, type ViewMarks } from "./editor-doc";
import { layoutFromPrefs, type DockLayout } from "./layout";
import { restoreSearchForm, toSearchParams } from "./search-params";
import type { ColorPrefs, DockingLayoutPrefs, MarkPrefs, Preferences, SearchWindowPrefs } from "./types";

const COLOR = "#9b2c1a";

export function defaultMarks(): MarkPrefs {
  return {
    whitespace: false,
    nbsp: false,
    bidi: false,
    glossary: true,
    translated: true,
    untranslated: true,
    noted: true,
    non_unique: false,
    auto_populated: true,
    alternative: true,
    paragraph_start: false,
    display_source: true,
    language_checker: false,
    font_fallback: false,
    modification: "none",
  };
}

export function defaultColors(): ColorPrefs {
  return { source: COLOR, target: COLOR, match_hit: COLOR, glossary: COLOR, nbsp: COLOR };
}

export function defaultDocking(): DockingLayoutPrefs {
  return {
    left: 0.25,
    notes: 0.2,
    editor_stack: 0.65,
    editor_main: 0.75,
    props: 0.5,
    matches: 0.8,
    east: 0.78,
    dict_mt: 0.5,
    show_dict: true,
    show_mt: true,
  };
}

export function defaultSearchWindow(): SearchWindowPrefs {
  return {
    search_type: "exact",
    case_sensitive: false,
    whole_word: false,
    source: true,
    translation: true,
    notes: false,
    comments: false,
    untranslated: false,
    author: "",
    date_from: "",
    date_to: "",
  };
}

export function defaultPreferences(partial?: Partial<Preferences>): Preferences {
  const base: Preferences = {
    theme: "light",
    locale: "en",
    autosave_seconds: 180,
    fuzzy_threshold: 30,
    insert_best_match: true,
    font_ui: "IBM Plex Sans",
    font_editor: "IBM Plex Sans",
    mt_enabled: [],
    config_dir: "",
    tab_advance: false,
    always_confirm_quit: false,
    first_time_wizard_done: true,
    colors: defaultColors(),
    export_tm_levels: "omegat level1 level2",
    tag_validation: "warn",
    filter_untranslated: false,
    matches_stemming_full: true,
    marks: defaultMarks(),
    project_files_show_translation_progress: true,
    project_files_show_on_load: false,
    remove_tags: false,
    spell_backend: "hunspell",
    languagetool_url: "",
    dictionary_dir: "dictionary",
    dictionary_fuzzy_matching: false,
    dictionary_auto_search: true,
    glossary_stem: true,
    glossary_ignore_case: true,
    glossary_not_exact_match: false,
    glossary_replace_on_insert: false,
    mt_auto_fetch: false,
    mt_keys: {},
    completer_auto: true,
    history_completion: true,
    history_prediction: true,
    completer_glossary: true,
    completer_tags: true,
    completer_autotext: true,
    completer_chartable: true,
    autotext: "",
    chartable: "©®™…—–«»",
    team_passphrase: "",
    team_conflict_resolution: "",
    plugin_dir: "plugins",
    version_check_enabled: true,
    secure_store_key: "",
    srx_path: "",
    srx_xml: "",
    finder_xml: "",
    script_dir: "scripts",
    script_slots: Array.from({ length: 12 }, () => ""),
    filter_options: {},
    filter_context: {},
    shortcuts: {},
    docking_layout: defaultDocking(),
    search_window: defaultSearchWindow(),
  };
  if (!partial) return base;
  return {
    ...base,
    ...partial,
    colors: { ...base.colors, ...partial.colors },
    marks: { ...base.marks, ...partial.marks },
    docking_layout: { ...base.docking_layout, ...partial.docking_layout },
    search_window: { ...base.search_window, ...partial.search_window },
    mt_keys: { ...base.mt_keys, ...partial.mt_keys },
    filter_options: { ...base.filter_options, ...partial.filter_options },
    filter_context: { ...base.filter_context, ...partial.filter_context },
    shortcuts: { ...base.shortcuts, ...partial.shortcuts },
    script_slots: partial.script_slots ?? base.script_slots,
    mt_enabled: partial.mt_enabled ?? base.mt_enabled,
  };
}

export function applyColorVars(colors: ColorPrefs) {
  if (typeof document === "undefined") return;
  const root = document.documentElement.style;
  root.setProperty("--color-source", colors.source);
  root.setProperty("--color-target", colors.target);
  root.setProperty("--color-match", colors.match_hit);
  root.setProperty("--color-glossary", colors.glossary);
  root.setProperty("--color-nbsp", colors.nbsp);
}

/** Every typed preference key and the observable effect it drives. */
export const PREF_CONSUMERS: Record<string, (p: Preferences) => unknown> = {
  theme: (p) => p.theme,
  locale: (p) => p.locale,
  autosave_seconds: (p) => p.autosave_seconds,
  fuzzy_threshold: (p) => p.fuzzy_threshold,
  insert_best_match: (p) => p.insert_best_match,
  font_ui: (p) => p.font_ui,
  font_editor: (p) => p.font_editor,
  mt_enabled: (p) => p.mt_enabled.join(","),
  config_dir: (p) => p.config_dir ?? "",
  tab_advance: (p) => (p.tab_advance ? "tab-commits" : "tab-completes"),
  always_confirm_quit: (p) => p.always_confirm_quit,
  first_time_wizard_done: (p) => p.first_time_wizard_done,
  "colors.source": (p) => p.colors.source,
  "colors.target": (p) => p.colors.target,
  "colors.match_hit": (p) => p.colors.match_hit,
  "colors.glossary": (p) => p.colors.glossary,
  "colors.nbsp": (p) => p.colors.nbsp,
  export_tm_levels: (p) => p.export_tm_levels,
  tag_validation: (p) => (p.tag_validation === "abort" ? "blocks-bad-tags" : "warns"),
  filter_untranslated: (p) => p.filter_untranslated,
  matches_stemming_full: (p) => p.matches_stemming_full,
  "marks.whitespace": (p) => decorateText("a b", marksFromPrefs(p.marks)).some((s) => s.cls.includes("mark-ws")),
  "marks.nbsp": (p) => decorateText("a\u00a0b", marksFromPrefs(p.marks)).some((s) => s.cls.includes("mark-nbsp")),
  "marks.bidi": (p) => decorateText("a\u200eb", marksFromPrefs(p.marks)).some((s) => s.cls.includes("mark-bidi")),
  "marks.glossary": (p) => decorateText("term", marksFromPrefs(p.marks), ["term"]).some((s) => s.cls.includes("mark-glossary")),
  "marks.translated": (p) => marksFromPrefs(p.marks).translated,
  "marks.untranslated": (p) => marksFromPrefs(p.marks).untranslated,
  "marks.noted": (p) => marksFromPrefs(p.marks).noted,
  "marks.non_unique": (p) => marksFromPrefs(p.marks).nonUnique,
  "marks.auto_populated": (p) => marksFromPrefs(p.marks).autoPopulated,
  "marks.alternative": (p) => marksFromPrefs(p.marks).alternative,
  "marks.paragraph_start": (p) => marksFromPrefs(p.marks).paragraphStart,
  "marks.display_source": (p) => marksFromPrefs(p.marks).displaySource,
  "marks.language_checker": (p) => marksFromPrefs(p.marks).languageChecker,
  "marks.font_fallback": (p) => marksFromPrefs(p.marks).fontFallback,
  "marks.modification": (p) => marksFromPrefs(p.marks).modification,
  project_files_show_translation_progress: (p) => p.project_files_show_translation_progress,
  project_files_show_on_load: (p) => p.project_files_show_on_load,
  remove_tags: (p) => p.remove_tags,
  spell_backend: (p) => p.spell_backend,
  languagetool_url: (p) => (p.languagetool_url ? "lt-remote" : "lt-degraded"),
  dictionary_dir: (p) => p.dictionary_dir,
  dictionary_fuzzy_matching: (p) => p.dictionary_fuzzy_matching,
  dictionary_auto_search: (p) => p.dictionary_auto_search,
  glossary_stem: (p) => p.glossary_stem,
  glossary_ignore_case: (p) => p.glossary_ignore_case,
  glossary_not_exact_match: (p) => p.glossary_not_exact_match,
  glossary_replace_on_insert: (p) => p.glossary_replace_on_insert,
  mt_auto_fetch: (p) => p.mt_auto_fetch,
  "mt_keys.google": (p) => p.mt_keys.google ?? "",
  completer_auto: (p) => p.completer_auto,
  history_completion: (p) => p.history_completion,
  history_prediction: (p) => p.history_prediction,
  completer_glossary: (p) => p.completer_glossary,
  completer_tags: (p) => p.completer_tags,
  completer_autotext: (p) => p.completer_autotext,
  completer_chartable: (p) => p.completer_chartable,
  autotext: (p) => p.autotext,
  chartable: (p) => p.chartable,
  team_passphrase: (p) => p.team_passphrase,
  team_conflict_resolution: (p) => p.team_conflict_resolution,
  plugin_dir: (p) => p.plugin_dir,
  version_check_enabled: (p) => p.version_check_enabled,
  secure_store_key: (p) => p.secure_store_key,
  srx_path: (p) => p.srx_path,
  srx_xml: (p) => p.srx_xml,
  finder_xml: (p) => p.finder_xml,
  script_dir: (p) => p.script_dir,
  "script_slots.0": (p) => p.script_slots[0] ?? "",
  "filter_options.po.skipHeader": (p) => p.filter_options.po?.skipHeader ?? "",
  "filter_context.segmentOn": (p) => p.filter_context.segmentOn ?? "",
  "shortcuts.project.save": (p) => p.shortcuts["project.save"] ?? "",
  "docking_layout.left": (p) => layoutFromPrefs(p.docking_layout).left,
  "docking_layout.show_mt": (p) => layoutFromPrefs(p.docking_layout).showMt,
  "search_window.notes": (p) => toSearchParams(restoreSearchForm(p.search_window)).notes,
  "search_window.search_type": (p) => toSearchParams(restoreSearchForm(p.search_window)).search_type,
};

export function consumePref(prefs: Preferences, key: string): unknown {
  const fn = PREF_CONSUMERS[key];
  if (!fn) throw new Error(`no consumer for ${key}`);
  return fn(prefs);
}

export function mutatePref(base: Preferences, key: string): Preferences {
  const next = defaultPreferences(base);
  switch (key) {
    case "theme":
      next.theme = "dark";
      break;
    case "locale":
      next.locale = "zh-CN";
      break;
    case "autosave_seconds":
      next.autosave_seconds = 30;
      break;
    case "fuzzy_threshold":
      next.fuzzy_threshold = 70;
      break;
    case "insert_best_match":
      next.insert_best_match = false;
      break;
    case "font_ui":
      next.font_ui = "Serif";
      break;
    case "font_editor":
      next.font_editor = "Mono";
      break;
    case "mt_enabled":
      next.mt_enabled = ["google"];
      break;
    case "config_dir":
      next.config_dir = "/tmp/omegat";
      break;
    case "tab_advance":
      next.tab_advance = true;
      break;
    case "always_confirm_quit":
      next.always_confirm_quit = true;
      break;
    case "first_time_wizard_done":
      next.first_time_wizard_done = false;
      break;
    case "colors.source":
      next.colors.source = "#0000ff";
      break;
    case "colors.target":
      next.colors.target = "#00ff00";
      break;
    case "colors.match_hit":
      next.colors.match_hit = "#ff00ff";
      break;
    case "colors.glossary":
      next.colors.glossary = "#00ffff";
      break;
    case "colors.nbsp":
      next.colors.nbsp = "#ffff00";
      break;
    case "export_tm_levels":
      next.export_tm_levels = "omegat";
      break;
    case "tag_validation":
      next.tag_validation = "abort";
      break;
    case "filter_untranslated":
      next.filter_untranslated = true;
      break;
    case "matches_stemming_full":
      next.matches_stemming_full = false;
      break;
    case "marks.whitespace":
      next.marks.whitespace = true;
      break;
    case "marks.nbsp":
      next.marks.nbsp = true;
      break;
    case "marks.bidi":
      next.marks.bidi = true;
      break;
    case "marks.glossary":
      next.marks.glossary = false;
      break;
    case "marks.translated":
      next.marks.translated = false;
      break;
    case "marks.untranslated":
      next.marks.untranslated = false;
      break;
    case "marks.noted":
      next.marks.noted = false;
      break;
    case "marks.non_unique":
      next.marks.non_unique = true;
      break;
    case "marks.auto_populated":
      next.marks.auto_populated = false;
      break;
    case "marks.alternative":
      next.marks.alternative = false;
      break;
    case "marks.paragraph_start":
      next.marks.paragraph_start = true;
      break;
    case "marks.display_source":
      next.marks.display_source = false;
      break;
    case "marks.language_checker":
      next.marks.language_checker = true;
      break;
    case "marks.font_fallback":
      next.marks.font_fallback = true;
      break;
    case "marks.modification":
      next.marks.modification = "all";
      break;
    case "project_files_show_translation_progress":
      next.project_files_show_translation_progress = false;
      break;
    case "project_files_show_on_load":
      next.project_files_show_on_load = true;
      break;
    case "remove_tags":
      next.remove_tags = true;
      break;
    case "spell_backend":
      next.spell_backend = "morfologik";
      break;
    case "languagetool_url":
      next.languagetool_url = "http://localhost:8081/v2/check";
      break;
    case "dictionary_dir":
      next.dictionary_dir = "dicts";
      break;
    case "dictionary_fuzzy_matching":
      next.dictionary_fuzzy_matching = true;
      break;
    case "dictionary_auto_search":
      next.dictionary_auto_search = false;
      break;
    case "glossary_stem":
      next.glossary_stem = false;
      break;
    case "glossary_ignore_case":
      next.glossary_ignore_case = false;
      break;
    case "glossary_not_exact_match":
      next.glossary_not_exact_match = true;
      break;
    case "glossary_replace_on_insert":
      next.glossary_replace_on_insert = true;
      break;
    case "mt_auto_fetch":
      next.mt_auto_fetch = true;
      break;
    case "mt_keys.google":
      next.mt_keys = { ...next.mt_keys, google: "k" };
      break;
    case "completer_auto":
      next.completer_auto = false;
      break;
    case "history_completion":
      next.history_completion = false;
      break;
    case "history_prediction":
      next.history_prediction = false;
      break;
    case "completer_glossary":
      next.completer_glossary = false;
      break;
    case "completer_tags":
      next.completer_tags = false;
      break;
    case "completer_autotext":
      next.completer_autotext = false;
      break;
    case "completer_chartable":
      next.completer_chartable = false;
      break;
    case "autotext":
      next.autotext = "omegat=OmegaT";
      break;
    case "chartable":
      next.chartable = "©";
      break;
    case "team_passphrase":
      next.team_passphrase = "secret";
      break;
    case "team_conflict_resolution":
      next.team_conflict_resolution = "theirs";
      break;
    case "plugin_dir":
      next.plugin_dir = "plug";
      break;
    case "version_check_enabled":
      next.version_check_enabled = false;
      break;
    case "secure_store_key":
      next.secure_store_key = "master";
      break;
    case "srx_path":
      next.srx_path = "custom.srx";
      break;
    case "srx_xml":
      next.srx_xml = "<srx/>";
      break;
    case "finder_xml":
      next.finder_xml = "<finder/>";
      break;
    case "script_dir":
      next.script_dir = "js";
      break;
    case "script_slots.0":
      next.script_slots = ["console.println(1)", ...next.script_slots.slice(1)];
      break;
    case "filter_options.po.skipHeader":
      next.filter_options = { po: { skipHeader: "true" } };
      break;
    case "filter_context.segmentOn":
      next.filter_context = { segmentOn: "BREAKS" };
      break;
    case "shortcuts.project.save":
      next.shortcuts = { "project.save": "Alt+S" };
      break;
    case "docking_layout.left":
      next.docking_layout = { ...next.docking_layout, left: 0.4 };
      break;
    case "docking_layout.show_mt":
      next.docking_layout = { ...next.docking_layout, show_mt: false };
      break;
    case "search_window.notes":
      next.search_window = { ...next.search_window, notes: true };
      break;
    case "search_window.search_type":
      next.search_window = { ...next.search_window, search_type: "regex" };
      break;
    default:
      throw new Error(`no mutator for ${key}`);
  }
  return next;
}

export type { DockLayout, ViewMarks };
