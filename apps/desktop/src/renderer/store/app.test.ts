import { describe, expect, it } from "vitest";
import { t, setLocale } from "../i18n";

describe("i18n", () => {
  it("falls back to english", () => {
    setLocale("en");
    expect(t("save")).toBe("Save");
  });
  it("switches to zh-CN", () => {
    setLocale("zh-CN");
    expect(t("save")).toBe("保存");
    setLocale("en");
  });
});
