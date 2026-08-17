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
export type MatchDto = {
  source: string;
  translation: string;
  score: number;
  score_no_stem?: number;
  adjusted_score?: number;
  comes_from: string;
  project?: string | null;
};
export type GlossaryHitDto = { source: string; target: string; comment: string };
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
};
export type IssueDto = { kind: string; index: number; file: string; message: string; severity: string };
export type SearchHitDto = { index: number; file: string; field: string; text: string };
export type CompleterItemDto = { kind: string; text: string; detail: string };
export type MtSuggestionDto = { engine: string; text: string };
export type DictHitDto = { word: string; definition: string; source: string };
export type FilterInfoDto = { id: string; name: string; masks: string[]; phase: number };
export type Preferences = {
  theme: string;
  locale: string;
  autosave_seconds: number;
  fuzzy_threshold: number;
  insert_best_match: boolean;
  font_ui: string;
  font_editor: string;
  mt_enabled: string[];
  extra: Record<string, string>;
};

declare global {
  interface Window {
    omegat: {
      rpc: (method: string, params?: unknown) => Promise<unknown>;
      pickDir: () => Promise<string | null>;
      pickFile: () => Promise<string | null>;
      onMenu: (channel: string, fn: (...args: unknown[]) => void) => () => void;
    };
  }
}

export {};
