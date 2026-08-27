export type VersionInfo = { name: string; version: string; protocol: string; rewrite: boolean };
export type EntryDto = {
  index: number;
  file: string;
  id: string;
  source: string;
  translation: string;
  note: string;
  comment: string;
  default_translation: boolean;
  revision: number;
  translated: boolean;
  tags: string[];
  properties: [string, string][];
};
export type EntrySetResult = {
  entry: EntryDto;
  updated: EntryDto[];
};
export type EditorConflict = {
  index: number;
  source: string;
  previous: string;
  ours: string;
  theirs: string;
  note: string;
  default_translation: boolean;
  remote_revision: number;
};
export type MatchDto = {
  source: string;
  translation: string;
  score: number;
  score_no_stem?: number;
  adjusted_score?: number;
  comes_from: string;
  project?: string | null;
  similarity?: number[];
};
export type GlossaryHitDto = { source: string; target: string; comment: string };
export type StatCountDto = {
  segments: number;
  words: number;
  "characters-without-spaces"?: number;
  characters: number;
  files: number;
};
export type FileStatDto = {
  filename: string;
  total: StatCountDto;
  remaining: StatCountDto;
  unique: StatCountDto;
  "unique-remaining"?: StatCountDto;
};
export type MatchBinDto = {
  exact: number;
  fuzzy_95: number;
  fuzzy_85: number;
  fuzzy_75: number;
  fuzzy_50: number;
  none: number;
};
export type StatsDto = {
  files: number;
  segments: number;
  translated: number;
  unique_segments: number;
  source_words: number;
  target_words: number;
  source_chars?: number;
  target_chars?: number;
  match_exact?: number;
  match_fuzzy?: number;
  match_none?: number;
  total?: StatCountDto;
  remaining?: StatCountDto;
  unique?: StatCountDto;
  "unique-remaining"?: StatCountDto;
  file_stats?: FileStatDto[];
  match_bins?: MatchBinDto;
};
export type ProjectPropsDto = {
  root: string;
  source_lang: string;
  target_lang: string;
  sentence_seg: boolean;
  source_dir?: string;
  target_dir?: string;
  tm_dir?: string;
  glossary_dir?: string;
  glossary_file?: string;
  dictionary_dir?: string;
  export_tm_levels?: string;
  support_default_translations?: boolean;
  remove_tags?: boolean;
  has_repositories: boolean;
  repositories?: RepositoryRow[];
};

export type RepositoryMappingRow = {
  local: string;
  repository: string;
  includes: string[];
  excludes: string[];
};

export type RepositoryRow = {
  repo_type: string;
  url: string;
  branch?: string | null;
  mappings: RepositoryMappingRow[];
};
export type IssueDto = { kind: string; index: number; file: string; message: string; severity: string };
export type SearchHitDto = {
  index: number;
  file: string;
  field: string;
  text: string;
  preview?: string | null;
};
export type SearchParams = {
  query: string;
  regex?: boolean;
  source?: boolean;
  translation?: boolean;
  notes?: boolean;
  comments?: boolean;
  glossary?: boolean;
  tmx?: boolean;
  case_sensitive?: boolean;
  whole_word?: boolean;
  untranslated?: boolean;
  search_type?: "exact" | "keyword" | "regex";
  author?: string;
  date_from?: string;
  date_to?: string;
  replace?: string;
  preview?: boolean;
};
export type CompleterItemDto = { kind: string; text: string; detail: string };
export type MtSuggestionDto = { engine: string; text: string };
export type DictHitDto = { word: string; definition: string; source: string };
export type FilterInfoDto = { id: string; name: string; masks: string[]; phase: number };
export type FilterOptionsDto = {
  id: string;
  name: string;
  masks: string[];
  phase: number;
  options: Record<string, string>;
};
export type MarkPrefs = {
  whitespace: boolean;
  nbsp: boolean;
  bidi: boolean;
  glossary: boolean;
  translated: boolean;
  untranslated: boolean;
  noted: boolean;
  non_unique: boolean;
  auto_populated: boolean;
  alternative: boolean;
  paragraph_start: boolean;
  display_source: boolean;
  language_checker: boolean;
  font_fallback: boolean;
  modification: "none" | "selected" | "all";
};
export type ColorPrefs = {
  source: string;
  target: string;
  match_hit: string;
  glossary: string;
  nbsp: string;
};
export type DockingLayoutPrefs = {
  left: number;
  notes: number;
  editor_stack: number;
  editor_main: number;
  props: number;
  matches: number;
  east: number;
  dict_mt: number;
  show_dict: boolean;
  show_mt: boolean;
};
export type SearchWindowPrefs = {
  search_type: "exact" | "keyword" | "regex";
  case_sensitive: boolean;
  whole_word: boolean;
  source: boolean;
  translation: boolean;
  notes: boolean;
  comments: boolean;
  untranslated: boolean;
  author: string;
  date_from: string;
  date_to: string;
};
export type Preferences = {
  theme: string;
  locale: string;
  autosave_seconds: number;
  fuzzy_threshold: number;
  insert_best_match: boolean;
  font_ui: string;
  font_editor: string;
  mt_enabled: string[];
  config_dir?: string;
  tab_advance: boolean;
  always_confirm_quit: boolean;
  first_time_wizard_done: boolean;
  colors: ColorPrefs;
  export_tm_levels: string;
  tag_validation: string;
  filter_untranslated: boolean;
  matches_stemming_full: boolean;
  marks: MarkPrefs;
  project_files_show_translation_progress: boolean;
  project_files_show_on_load: boolean;
  remove_tags: boolean;
  spell_backend: string;
  languagetool_url: string;
  dictionary_dir: string;
  dictionary_fuzzy_matching: boolean;
  dictionary_auto_search: boolean;
  glossary_stem: boolean;
  glossary_ignore_case: boolean;
  glossary_not_exact_match: boolean;
  glossary_replace_on_insert: boolean;
  mt_auto_fetch: boolean;
  mt_keys: Record<string, string>;
  completer_auto: boolean;
  history_completion: boolean;
  history_prediction: boolean;
  completer_glossary: boolean;
  completer_tags: boolean;
  completer_autotext: boolean;
  completer_chartable: boolean;
  autotext: string;
  chartable: string;
  team_passphrase: string;
  team_conflict_resolution: string;
  plugin_dir: string;
  version_check_enabled: boolean;
  secure_store_key: string;
  srx_path: string;
  srx_xml: string;
  finder_xml: string;
  script_dir: string;
  script_slots: string[];
  filter_options: Record<string, Record<string, string>>;
  filter_context: Record<string, string>;
  shortcuts: Record<string, string>;
  docking_layout: DockingLayoutPrefs;
  search_window: SearchWindowPrefs;
  controller_keys: Record<string, string>;
};
export type TeamConflict = {
  kind?: string;
  source?: string;
  ours?: string;
  theirs?: string;
  message?: string;
};
export type WindowId =
  | "search"
  | "replace"
  | "prefs"
  | "about"
  | "license"
  | "log"
  | "align"
  | "team"
  | "files"
  | "issues"
  | "wizard"
  | "project-edit"
  | "finder"
  | "tip"
  | "stats-standard"
  | "stats-matches"
  | "stats-files"
  | "filters"
  | "segmentation"
  | "shortcuts"
  | "glossary-add"
  | "wiki"
  | "med"
  | "convert"
  | "scripts"
  | "changes"
  | "mapping";

declare global {
  interface Window {
    omegat: {
      rpc: (method: string, params?: unknown) => Promise<unknown>;
      startup?: () => Promise<{
        project: string | null;
        configDir: string;
        scriptsDir: string | null;
      }>;
      pickDir: () => Promise<string | null>;
      pickFile: () => Promise<string | null>;
      pickFiles?: () => Promise<string[] | null>;
      saveText?: (name: string, text: string) => Promise<string | null>;
      quit?: () => Promise<void>;
      relaunch?: () => Promise<void>;
      openPath: (path: string) => Promise<void>;
      openExternal: (url: string) => Promise<void>;
      openManual?: () => Promise<void>;
      setMenuLocale?: (locale: string) => Promise<void>;
      onMenu: (channel: string, fn: (...args: unknown[]) => void) => () => void;
    };
  }
}

export {};
