import en from "./en.json";
import zhCN from "./zh-CN.json";
import de from "./de.json";
import fr from "./fr.json";
import ja from "./ja.json";
import es from "./es.json";
import ru from "./ru.json";

const catalogs: Record<string, Record<string, string>> = {
  en,
  "zh-CN": zhCN,
  zh: zhCN,
  de,
  fr,
  ja,
  es,
  ru,
};

let locale = "en";

export function setLocale(next: string) {
  locale = catalogs[next] ? next : "en";
}

export function t(key: string): string {
  return catalogs[locale]?.[key] ?? catalogs.en[key] ?? key;
}

export function currentLocale() {
  return locale;
}
