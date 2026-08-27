import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { PREF_PAGES } from "./pages";

describe("25 Java preference controllers", () => {
  it("each exported controller has a page and its keys appear in pages.tsx", () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const gold = JSON.parse(
      readFileSync(join(here, "../../../../../fixtures/goldens/engine/preference_keys.json"), "utf8"),
    ) as { controllers: Record<string, string[]> };
    const src = readFileSync(join(here, "pages.tsx"), "utf8");
    const controllers = Object.keys(gold.controllers);
    expect(controllers.length).toBe(25);
    expect(PREF_PAGES.length).toBeGreaterThanOrEqual(25);
    const missing: string[] = [];
    for (const [name, keys] of Object.entries(gold.controllers)) {
      for (const key of keys) {
        if (!key || key.startsWith("—")) continue;
        if (!src.includes(key) && !src.includes(key.replace(/-/g, "_"))) {
          missing.push(`${name}:${key}`);
        }
      }
    }
    expect(missing).toEqual([]);
  });
});
