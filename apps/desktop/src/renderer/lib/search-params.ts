import type { SearchParams } from "./types";

export type SearchForm = {
  query: string;
  replace: string;
  searchType: "exact" | "keyword" | "regex";
  source: boolean;
  translation: boolean;
  notes: boolean;
  comments: boolean;
  caseSensitive: boolean;
  wholeWord: boolean;
  untranslated: boolean;
  author: string;
  dateFrom: string;
  dateTo: string;
};

export function defaultSearchForm(): SearchForm {
  return {
    query: "",
    replace: "",
    searchType: "exact",
    source: true,
    translation: true,
    notes: false,
    comments: false,
    caseSensitive: false,
    wholeWord: false,
    untranslated: false,
    author: "",
    dateFrom: "",
    dateTo: "",
  };
}

export function toSearchParams(form: SearchForm, opts?: { preview?: boolean; withReplace?: boolean }): SearchParams {
  return {
    query: form.query,
    regex: form.searchType === "regex",
    search_type: form.searchType,
    source: form.source,
    translation: form.translation,
    notes: form.notes,
    comments: form.comments,
    case_sensitive: form.caseSensitive,
    whole_word: form.wholeWord,
    untranslated: form.untranslated,
    author: form.author || undefined,
    date_from: form.dateFrom || undefined,
    date_to: form.dateTo || undefined,
    replace: opts?.withReplace && form.replace.length > 0 ? form.replace : undefined,
    preview: Boolean(opts?.preview),
  };
}

export function persistSearchForm(form: SearchForm): Record<string, string> {
  return {
    search_window_search_type: form.searchType,
    search_window_case_sensitive: String(form.caseSensitive),
    search_window_whole_words: String(form.wholeWord),
    search_window_search_source: String(form.source),
    search_window_search_translation: String(form.translation),
    search_window_search_notes: String(form.notes),
    search_window_search_comments: String(form.comments),
    search_window_replace_untranslated: String(form.untranslated),
    search_window_author_name: form.author,
    search_window_date_from_value: form.dateFrom,
    search_window_date_to_value: form.dateTo,
  };
}

export function restoreSearchForm(extra: Record<string, string> | undefined): SearchForm {
  const base = defaultSearchForm();
  if (!extra) return base;
  const type = extra.search_window_search_type;
  return {
    ...base,
    searchType: type === "keyword" || type === "regex" ? type : "exact",
    caseSensitive: extra.search_window_case_sensitive === "true",
    wholeWord: extra.search_window_whole_words === "true",
    source: extra.search_window_search_source !== "false",
    translation: extra.search_window_search_translation !== "false",
    notes: extra.search_window_search_notes === "true",
    comments: extra.search_window_search_comments === "true",
    untranslated: extra.search_window_replace_untranslated === "true",
    author: extra.search_window_author_name ?? "",
    dateFrom: extra.search_window_date_from_value ?? "",
    dateTo: extra.search_window_date_to_value ?? "",
  };
}
