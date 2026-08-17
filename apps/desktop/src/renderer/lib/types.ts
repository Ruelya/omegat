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
  comes_from: string;
};
export type GlossaryHitDto = { source: string; target: string; comment: string };
export type StatsDto = {
  files: number;
  segments: number;
  translated: number;
  unique_segments: number;
  source_words: number;
  target_words: number;
};
export type ProjectPropsDto = {
  root: string;
  source_lang: string;
  target_lang: string;
  sentence_seg: boolean;
  has_repositories: boolean;
};
export type IssueDto = { kind: string; index: number; file: string; message: string; severity: string };
export type SearchHitDto = { index: number; file: string; field: string; text: string };

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
