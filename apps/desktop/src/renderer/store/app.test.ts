import { describe, expect, it } from "vitest";
import { availableLocales, detectLocale, isRtl, setLocale, t } from "../i18n";

describe("i18n", () => {
  it("ships 41 UI catalogs", () => {
    expect(availableLocales()).toHaveLength(41);
  });
  it("falls back to english", () => {
    setLocale("en");
    expect(t("save")).toBe("Save");
  });
  it("switches to zh-CN", () => {
    setLocale("zh-CN");
    expect(t("save")).toBe("保存");
    setLocale("en");
  });
  it("detects locales and RTL", () => {
    expect(detectLocale("zh_TW")).toBe("zh-TW");
    expect(detectLocale("pt-BR")).toBe("pt-BR");
    expect(detectLocale("ar-EG")).toBe("ar");
    expect(detectLocale("xx-YY")).toBe("en");
    expect(isRtl("ar")).toBe(true);
    expect(isRtl("en")).toBe(false);
  });
});
