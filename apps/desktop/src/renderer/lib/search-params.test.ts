import { describe, expect, it } from "vitest";
import { defaultSearchForm, persistSearchForm, restoreSearchForm, toSearchParams } from "./search-params";

describe("search params", () => {
  it("maps Java SearchWindow fields onto RPC", () => {
    const form = {
      ...defaultSearchForm(),
      query: "hello",
      replace: "bonjour",
      searchType: "keyword" as const,
      notes: true,
      comments: true,
      caseSensitive: true,
      wholeWord: true,
      untranslated: true,
      author: "alice",
      dateFrom: "20200101",
      dateTo: "20201231",
    };
    const rpc = toSearchParams(form, { withReplace: true, preview: true });
    expect(rpc).toMatchObject({
      query: "hello",
      search_type: "keyword",
      notes: true,
      comments: true,
      case_sensitive: true,
      whole_word: true,
      untranslated: true,
      author: "alice",
      date_from: "20200101",
      date_to: "20201231",
      replace: "bonjour",
      preview: true,
      regex: false,
    });
  });

  it("round-trips typed SearchWindowPrefs", () => {
    const form = { ...defaultSearchForm(), notes: true, searchType: "regex" as const };
    const saved = persistSearchForm(form);
    expect(saved.notes).toBe(true);
    expect(saved.search_type).toBe("regex");
    const restored = restoreSearchForm(saved);
    expect(restored.notes).toBe(true);
    expect(restored.searchType).toBe("regex");
    expect(toSearchParams(restored).regex).toBe(true);
  });
});
