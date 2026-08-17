import { describe, expect, it } from "vitest";
import { consumePref, defaultPreferences, mutatePref, PREF_CONSUMERS } from "./preferences";
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
    expect(PREF_PAGES).toHaveLength(28);
    expect(new Set(PREF_PAGES.map((p) => p.id)).size).toBe(28);
  });
});
