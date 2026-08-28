/** Java `org.omegat.gui.glossary.GlossaryAutoCompleterView`. */
import { AutoCompleterListView } from "./AutoCompleterListView";
import { item, type AutoCompleterItem } from "./AutoCompleterItem";

export type GlossaryEntry = { source: string; locTerms: string[] };

function isLetter(ch: string): boolean {
  return /\p{L}/u.test(ch);
}
function isUpper(ch: string): boolean {
  return ch === ch.toUpperCase() && ch !== ch.toLowerCase();
}
function isLower(ch: string): boolean {
  return ch === ch.toLowerCase() && ch !== ch.toUpperCase();
}
function isTitleCase(s: string): boolean {
  const chars = [...s];
  if (!chars[0] || !isUpper(chars[0])) return false;
  if (chars.length === 1) return true;
  return chars.slice(1).every((c) => !isLetter(c) || isLower(c));
}
function isLowerCase(s: string): boolean {
  return [...s].filter(isLetter).every(isLower);
}
function isUpperCase(s: string): boolean {
  const letters = [...s].filter(isLetter);
  return letters.length >= 2 && letters.every(isUpper);
}

/** Java `StringUtil.matchCapitalization`. */
export function matchCapitalization(term: string, context: string | null): string {
  if (!context || !term) return term;
  if (term.startsWith(context)) return term;
  if (isTitleCase(context)) {
    const chars = [...term];
    if (!chars[0]) return term;
    return chars[0].toUpperCase() + chars.slice(1).join("");
  }
  if (isLowerCase(context)) return term.toLowerCase();
  if (isUpperCase(context)) return term.toUpperCase();
  return term;
}

export function getLastToken(prevText: string): string {
  const m = /([^\s]+)$/.exec(prevText);
  return m?.[1] ?? "";
}

export class GlossaryAutoCompleterView extends AutoCompleterListView {
  entries: GlossaryEntry[] = [];
  /** Compatibility with earlier tests that used `{source,target}`. */
  set terms(value: { source: string; target: string }[]) {
    this.entries = value.map((t) => ({ source: t.source, locTerms: [t.target] }));
  }
  get terms(): { source: string; target: string }[] {
    return this.entries.map((e) => ({ source: e.source, target: e.locTerms[0] ?? "" }));
  }

  constructor() {
    super("glossary");
  }

  computeListData(prevText: string, contextualOnly = false): AutoCompleterItem[] {
    const wordChunk = getLastToken(prevText);
    let sortMatchTo: string | null = wordChunk;
    const result: AutoCompleterItem[] = [];
    this.fillMatchingTerms(result, this.entries, wordChunk);
    if (result.length === 0 && !contextualOnly) {
      this.fillMatchingTerms(result, this.entries, null);
      sortMatchTo = null;
    }
    result.sort((a, b) => this.compare(a, b, sortMatchTo));
    return result;
  }

  shouldPopUp(wordChunk: string): boolean {
    const leading = getLastToken(wordChunk);
    const entries = this.computeListData(leading, true);
    return entries.length > 0 && (leading.length > 1 || entries.length <= 10);
  }

  private fillMatchingTerms(result: AutoCompleterItem[], glossary: GlossaryEntry[], context: string | null): void {
    if (context === "") return;
    for (const entry of glossary) {
      for (const term of entry.locTerms) {
        if (!this.termMatchesChunk(term, context)) continue;
        const length = context == null ? 0 : context.length;
        const payload = matchCapitalization(term, context);
        const next = item(payload, "glossary", [entry.source], -length);
        if (!result.some((r) => r.payload === next.payload && r.extras[0] === next.extras[0])) {
          result.push(next);
        }
        if (payload !== term) {
          result.push(item(term, "glossary", [entry.source], -length));
        }
      }
    }
  }

  private termMatchesChunk(term: string, context: string | null): boolean {
    if (context == null) return true;
    const lowerTerm = term.toLowerCase();
    const lowerContext = context.toLowerCase();
    return lowerTerm !== lowerContext && lowerTerm.startsWith(lowerContext);
  }

  private compare(o1: AutoCompleterItem, o2: AutoCompleterItem, matchTo: string | null): number {
    if (matchTo) {
      const o1Matches = o1.payload.startsWith(matchTo);
      const o2Matches = o2.payload.startsWith(matchTo);
      if (o1Matches && !o2Matches) return -1;
      if (!o1Matches && o2Matches) return 1;
      const o1Orig = this.isOriginal(o1);
      const o2Orig = this.isOriginal(o2);
      if (o1Orig && !o2Orig) return -1;
      if (!o1Orig && o2Orig) return 1;
    }
    let i1 = -1;
    let i2 = -1;
    for (let i = 0; i < this.entries.length; i++) {
      if (this.entries[i]!.source === o1.extras[0]) i1 = i;
      if (this.entries[i]!.source === o2.extras[0]) i2 = i;
      if (i1 !== -1 && i2 !== -1) break;
    }
    return i1 - i2;
  }

  private isOriginal(item: AutoCompleterItem): boolean {
    return this.entries.some((e) => e.locTerms.includes(item.payload));
  }
}
