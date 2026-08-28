import type { SearchParams, SearchWindowPrefs } from "./types";

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

export function persistSearchForm(form: SearchForm): SearchWindowPrefs {
  return {
    search_type: form.searchType,
    case_sensitive: form.caseSensitive,
    whole_word: form.wholeWord,
    source: form.source,
    translation: form.translation,
    notes: form.notes,
    comments: form.comments,
    untranslated: form.untranslated,
    author: form.author,
    date_from: form.dateFrom,
    date_to: form.dateTo,
  };
}

export function restoreSearchForm(saved: SearchWindowPrefs | undefined): SearchForm {
  const base = defaultSearchForm();
  if (!saved) return base;
  const type = saved.search_type;
  return {
    ...base,
    searchType: type === "keyword" || type === "regex" ? type : "exact",
    caseSensitive: saved.case_sensitive,
    wholeWord: saved.whole_word,
    source: saved.source,
    translation: saved.translation,
    notes: saved.notes,
    comments: saved.comments,
    untranslated: saved.untranslated,
    author: saved.author ?? "",
    dateFrom: saved.date_from ?? "",
    dateTo: saved.date_to ?? "",
  };
}
