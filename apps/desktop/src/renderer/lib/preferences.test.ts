import { describe, expect, it } from "vitest";
import {
  consumePref,
  defaultPreferences,
  mutatePref,
  preferenceMergePatch,
  PREF_CONSUMERS,
} from "./preferences";
import { PREF_PAGES } from "../prefs/pages";

describe("typed preferences", () => {
  it("has a consumer for every preference key", () => {
    const keys = Object.keys(PREF_CONSUMERS);
    expect(keys.length).toBeGreaterThan(40);
    const base = defaultPreferences();
    for (const key of keys) {
      const mutated = mutatePref(base, key);
      expect(consumePref(mutated, key), key).not.toEqual(consumePref(base, key));
    }
  });

  it("registers 28 preference controllers", () => {
    expect(PREF_PAGES).toHaveLength(29);
    expect(new Set(PREF_PAGES.map((p) => p.id)).size).toBe(29);
  });

  it("diffs edited snapshots down to independent nested leaves", () => {
    const base = defaultPreferences({
      filter_options: {
        text: { preserve_spaces: "true", encoding: "utf8" },
        po: { skip_header: "false" },
      },
    });
    const desired = defaultPreferences({
      ...base,
      locale: "fr",
      filter_options: {
        ...base.filter_options,
        text: { ...base.filter_options.text, preserve_spaces: "false" },
      },
    });
    expect(preferenceMergePatch(base, desired)).toEqual({
      locale: "fr",
      filter_options: { text: { preserve_spaces: "false" } },
    });
  });
});
