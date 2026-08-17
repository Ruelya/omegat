import ar from "./ar.json";
import be from "./be.json";
import ca from "./ca.json";
import co from "./co.json";
import cs from "./cs.json";
import cy from "./cy.json";
import da from "./da.json";
import de from "./de.json";
import el from "./el.json";
import en from "./en.json";
import eo from "./eo.json";
import es from "./es.json";
import eu from "./eu.json";
import fi from "./fi.json";
import fr from "./fr.json";
import gl from "./gl.json";
import hr from "./hr.json";
import hu from "./hu.json";
import ia from "./ia.json";
import id from "./id.json";
import it from "./it.json";
import ja from "./ja.json";
import ko from "./ko.json";
import mfe from "./mfe.json";
import nl from "./nl.json";
import no from "./no.json";
import pl from "./pl.json";
import pt from "./pt.json";
import pt_BR from "./pt-BR.json";
import ru from "./ru.json";
import sc from "./sc.json";
import sh from "./sh.json";
import sk from "./sk.json";
import sl from "./sl.json";
import sq from "./sq.json";
import sv from "./sv.json";
import tk from "./tk.json";
import tr from "./tr.json";
import uk from "./uk.json";
import zh_CN from "./zh-CN.json";
import zh_TW from "./zh-TW.json";

const catalogs: Record<string, Record<string, string>> = {
  ar,
  be,
  ca,
  co,
  cs,
  cy,
  da,
  de,
  el,
  en,
  eo,
  es,
  eu,
  fi,
  fr,
  gl,
  hr,
  hu,
  ia,
  id,
  it,
  ja,
  ko,
  mfe,
  nl,
  no,
  pl,
  pt,
  "pt-BR": pt_BR,
  ru,
  sc,
  sh,
  sk,
  sl,
  sq,
  sv,
  tk,
  tr,
  uk,
  "zh-CN": zh_CN,
  "zh-TW": zh_TW,
};

const RTL = new Set(["ar"]);

let locale = "en";

export function availableLocales(): string[] {
  return Object.keys(catalogs).sort();
}

export function detectLocale(tag: string): string {
  const n = (tag || "en").replaceAll("_", "-");
  if (catalogs[n]) return n;
  const lower = n.toLowerCase();
  const base = lower.split("-")[0] ?? "en";
  if (base === "zh") {
    return /tw|hk|hant|mo/.test(lower) ? "zh-TW" : "zh-CN";
  }
  if (base === "pt" && /br/.test(lower)) return "pt-BR";
  if (catalogs[base]) return base;
  return "en";
}

export function isRtl(loc: string = locale): boolean {
  return RTL.has(loc);
}

export function applyDocumentLocale(next: string) {
  const loc = catalogs[next] ? next : "en";
  locale = loc;
  if (typeof document !== "undefined") {
    document.documentElement.lang = loc;
    document.documentElement.dir = isRtl(loc) ? "rtl" : "ltr";
  }
}

export function setLocale(next: string) {
  applyDocumentLocale(catalogs[next] ? next : detectLocale(next));
}

export function t(key: string): string {
  return catalogs[locale]?.[key] ?? catalogs.en[key] ?? key;
}

export function currentLocale() {
  return locale;
}
