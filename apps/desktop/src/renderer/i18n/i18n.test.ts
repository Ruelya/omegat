import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { availableLocales, detectLocale, isRtl, setLocale, t } from "./index";

const dir = dirname(fileURLToPath(import.meta.url));

function load(name: string): Record<string, string> {
  return JSON.parse(readFileSync(join(dir, name), "utf8")) as Record<string, string>;
}

describe("UI locales", () => {
  const en = load("en.json");
  const files = readdirSync(dir).filter((f) => f.endsWith(".json"));

  it("ships 41 catalogs with the same keys as en.json", () => {
    expect(files).toHaveLength(41);
    expect(availableLocales()).toHaveLength(41);
    const keys = Object.keys(en).sort();
    expect(keys.length).toBeGreaterThan(130);
    for (const file of files) {
      const cat = load(file);
      expect(Object.keys(cat).sort(), file).toEqual(keys);
    }
  });

  it("migrates create by UI key, never BUTTON_ADD", () => {
    expect(en.create).toBe("Create");
    expect(load("zh-CN.json").create).toBe("创建");
    expect(load("zh-TW.json").create).toBe("建立");
    expect(load("ja.json").create).toBe("作成");
    expect(load("de.json").create).toBe("Erstellen");
    expect(load("ar.json").create).toBe("إنشاء");
    expect(load("zh-CN.json").create).not.toBe("添加");
  });

  it("has no leftover Auto-completion English tail", () => {
    for (const file of files) {
      const cat = load(file);
      expect(JSON.stringify(cat).includes("Auto-completion"), file).toBe(false);
      expect(cat.completer.length).toBeGreaterThan(0);
    }
    expect(en.completer).toBe("Auto-Completion");
    expect(load("zh-CN.json").completer).toBe("自动完成");
    expect(load("de.json").completer).not.toBe("Auto-completion");
  });

  it("applies ar as RTL and keeps native menu keys", () => {
    expect(isRtl("ar")).toBe(true);
    expect(isRtl("zh-CN")).toBe(false);
    expect(detectLocale("zh-TW")).toBe("zh-TW");
    expect(detectLocale("pt-BR")).toBe("pt-BR");
    setLocale("zh-CN");
    expect(t("create")).toBe("创建");
    expect(t("menuProject")).toBeTruthy();
    expect(t("menuProject")).not.toBe("menuProject");
    setLocale("en");
  });
});
