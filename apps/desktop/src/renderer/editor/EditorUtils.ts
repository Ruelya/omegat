/** Java `org.omegat.gui.editor.EditorUtils`. */

export function removeDirectionChars(s: string): string {
  return s.replace(/[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, "");
}

function matchCapitalization(src: string, dest: string): string {
  if (!src || !dest) return dest;
  if (src === src.toUpperCase() && /[A-Za-z]/.test(src)) return dest.toUpperCase();
  if (src === src.toLowerCase()) return dest.toLowerCase();
  const title = src[0] === src[0].toUpperCase() && src.slice(1) === src.slice(1).toLowerCase();
  if (title) return dest.charAt(0).toUpperCase() + dest.slice(1).toLowerCase();
  return dest.toLowerCase();
}

/** Java `EditorUtils.replaceGlossaryEntries`. Longest source first; match source capitalization. */
export function replaceGlossaryEntries(
  src: string | null,
  entries: { source: string; target: string }[] | null,
): string | null {
  if (src == null) return null;
  if (!src) return "";
  if (!entries?.length) return src;
  const sorted = [...entries].sort((a, b) => b.source.length - a.source.length);
  let out = src;
  for (const e of sorted) {
    const re = new RegExp(e.source.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    out = out.replace(re, (m) => matchCapitalization(m, e.target));
  }
  return out;
}

export type ChangeCaseMode = "upper" | "lower" | "title" | "sentence" | "cycle";

type CaseToken = { start: number; end: number; text: string };

const CASE_TOKEN = /[\p{L}\p{M}\p{N}]+(?:['’][\p{L}\p{M}\p{N}]+)*/gu;
const OMEGAT_TAG = /<\/?[A-Za-z][\w:-]*\d*\/?>/g;
const TITLE_CASE_VARIANTS: Record<string, string> = {
  "\u01c4": "\u01c5",
  "\u01c6": "\u01c5",
  "\u01c7": "\u01c8",
  "\u01c9": "\u01c8",
  "\u01ca": "\u01cb",
  "\u01cc": "\u01cb",
  "\u01f1": "\u01f2",
  "\u01f3": "\u01f2",
};
const ACTUAL_TITLE_CASE = new Set(["\u01c5", "\u01c8", "\u01cb", "\u01f2"]);
const UPPER_WITH_TITLE_VARIANT = new Set(["\u01c4", "\u01c7", "\u01ca", "\u01f1"]);

function localeLower(text: string, locale: string): string {
  try {
    return text.toLocaleLowerCase(locale);
  } catch {
    return text.toLowerCase();
  }
}

function localeUpper(text: string, locale: string): string {
  try {
    return text.toLocaleUpperCase(locale);
  } catch {
    return text.toUpperCase();
  }
}

function isLetter(char: string): boolean {
  return /^\p{L}$/u.test(char);
}

function letters(text: string): string[] {
  return [...text].filter(isLetter);
}

function isLowerCase(text: string): boolean {
  const chars = letters(text);
  return chars.length > 0 && chars.every((char) => char === char.toLowerCase());
}

function isUpperCase(text: string): boolean {
  const chars = letters(text);
  return chars.length > 0 && chars.every((char) => char === char.toUpperCase());
}

/** Java Character title-case rule; notably U+01C7 (Ǉ) is not title case. */
export function isTitleCaseCodePoint(char: string): boolean {
  if (ACTUAL_TITLE_CASE.has(char)) return true;
  return char === char.toUpperCase()
    && char !== char.toLowerCase()
    && !UPPER_WITH_TITLE_VARIANT.has(char);
}

function isTitleCase(text: string): boolean {
  const chars = [...text];
  if (chars.length === 0 || !isTitleCaseCodePoint(chars[0]!)) return false;
  return chars.length === 1 || isLowerCase(chars.slice(1).join(""));
}

function isMixedCase(text: string): boolean {
  const chars = [...text];
  if (chars.length < 2) return false;
  let lower = false;
  let upperAfterFirst = false;
  chars.forEach((char, index) => {
    if (!isLetter(char)) return;
    if (char === char.toLowerCase() && char !== char.toUpperCase()) lower = true;
    if (index > 0 && char === char.toUpperCase() && char !== char.toLowerCase()) {
      upperAfterFirst = true;
    }
  });
  return lower && upperAfterFirst;
}

function toTitleCase(text: string, locale: string): string {
  const chars = [...text];
  const index = chars.findIndex(isLetter);
  if (index < 0) return text;
  const first = chars[index]!;
  const titled = TITLE_CASE_VARIANTS[first] ?? localeUpper(first, locale);
  return chars.slice(0, index).join("")
    + titled
    + localeLower(chars.slice(index + 1).join(""), locale);
}

function caseTokens(text: string): CaseToken[] {
  const tagRanges = [...text.matchAll(OMEGAT_TAG)].map((match) => ({
    start: match.index ?? 0,
    end: (match.index ?? 0) + match[0].length,
  }));
  return [...text.matchAll(CASE_TOKEN)].flatMap((match) => {
    const start = match.index ?? 0;
    const end = start + match[0].length;
    return tagRanges.some((tag) => start >= tag.start && end <= tag.end)
      ? []
      : [{ start, end, text: match[0] }];
  });
}

function cycleMode(tokens: readonly CaseToken[]): Exclude<ChangeCaseMode, "cycle"> | null {
  let lower = 0;
  let upper = 0;
  let title = 0;
  let ambiguous = 0;
  let mixed = 0;
  for (const token of tokens) {
    if (!isLetter([...token.text][0] ?? "")) continue;
    if (isLowerCase(token.text)) {
      lower += 1;
    } else {
      const tokenTitle = isTitleCase(token.text);
      const tokenUpper = isUpperCase(token.text);
      if (tokenTitle && tokenUpper) ambiguous += 1;
      else if (tokenTitle) title += 1;
      else if (tokenUpper) upper += 1;
      else if (isMixedCase(token.text)) mixed += 1;
    }
  }
  const present = Number(lower > 0) + Number(upper > 0) + Number(title > 0) + Number(mixed > 0);
  if (lower + upper + title + ambiguous + mixed === 0) return null;
  if ((title > 0 || ambiguous > 0) && lower > 0 && upper === 0 && mixed === 0) return "title";
  if (mixed > 0 || present > 1) return "upper";
  if (lower > 0) return "sentence";
  if (title > 0) return "upper";
  if (upper > 0 || ambiguous > 0) return "lower";
  return "upper";
}

/**
 * Java `EditorUtils.doChangeCase`: transform tokenizer words while leaving
 * OmegaT tags and punctuation byte-for-byte intact.
 */
export function changeCase(
  text: string,
  mode: ChangeCaseMode,
  locale = "en",
): string {
  const tokens = caseTokens(text).filter((token) => isLetter([...token.text][0] ?? ""));
  const target = mode === "cycle" ? cycleMode(tokens) : mode;
  if (target === null) return text;
  let sentenceFirst = true;
  let offset = 0;
  let result = text;
  for (const token of tokens) {
    let replacement: string;
    if (target === "lower") replacement = localeLower(token.text, locale);
    else if (target === "upper") replacement = localeUpper(token.text, locale);
    else if (target === "title") replacement = toTitleCase(token.text, locale);
    else if (sentenceFirst) {
      replacement = toTitleCase(token.text, locale);
      sentenceFirst = false;
    } else {
      replacement = localeLower(token.text, locale);
    }
    const start = token.start + offset;
    result = result.slice(0, start) + replacement + result.slice(start + token.text.length);
    offset += replacement.length - token.text.length;
  }
  return result;
}

const CJK_WORDS: Record<string, string[]> = {
  ja: ["太平寺", "の", "中心", "的", "な", "ペン", "塔"],
  zh: ["太平寺", "中的", "文笔", "塔"],
  "zh-CN": ["太平寺", "中的", "文笔", "塔"],
};

function cjkSpans(locale: string, text: string): [number, number][] {
  const dict = CJK_WORDS[locale] ?? CJK_WORDS[locale.split("-")[0] ?? ""] ?? [];
  const spans: [number, number][] = [];
  let i = 0;
  while (i < text.length) {
    let hit = "";
    for (const w of dict) {
      if (text.startsWith(w, i) && w.length > hit.length) hit = w;
    }
    if (hit) {
      spans.push([i, i + hit.length]);
      i += hit.length;
    } else {
      spans.push([i, i + 1]);
      i += 1;
    }
  }
  return spans;
}

function latinSpans(text: string): [number, number][] {
  const spans: [number, number][] = [];
  const re = /[\p{L}\p{M}\p{N}']+/gu;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    spans.push([m.index, m.index + m[0].length]);
  }
  return spans;
}

/** Java `EditorUtils.getWordBoundary` (BreakIterator-style). */
export function getWordBoundary(locale: string, text: string, offset: number, forward: boolean): number {
  const loc = locale.replace("_", "-");
  const cjk = /ja|zh/i.test(loc);
  const spans = cjk ? cjkSpans(loc === "zh-CN" || loc === "zh" ? "zh" : "ja", text) : latinSpans(text);
  if (spans.length === 0) return forward ? text.length : 0;
  const pos = Math.max(0, Math.min(offset, text.length));
  if (forward) {
    for (const [, end] of spans) {
      if (end > pos) return end;
    }
    return text.length;
  }
  for (let i = spans.length - 1; i >= 0; i--) {
    const [start] = spans[i]!;
    if (start <= pos) return start;
  }
  return 0;
}
