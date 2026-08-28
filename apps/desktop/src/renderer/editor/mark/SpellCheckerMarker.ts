/** Java `org.omegat.core.spellchecker.SpellCheckerMarker`. */
import type { IAsyncMarker, MarkerInput } from "./IMarker";
import type { Mark } from "./Mark";

export type SpellToken = {
  word: string;
  offset: number;
  length: number;
};

export type SpellCheckClient = (text: string) => Promise<SpellToken[]>;

async function sidecarSpellCheck(text: string): Promise<SpellToken[]> {
  if (typeof window === "undefined" || !window.omegat) return [];
  return window.omegat.rpc("spell.check", { text }) as Promise<SpellToken[]>;
}

export class SpellCheckerMarker implements IAsyncMarker {
  constructor(private readonly check: SpellCheckClient = sidecarSpellCheck) {}

  async getMarksForEntryAsync(input: MarkerInput): Promise<Mark[] | null> {
    if (input.translationText == null || input.enabled === false) return null;
    const tokens = await this.check(input.translationText);
    return tokens.map((token) => ({
      startOffset: token.offset,
      endOffset: token.offset + token.length,
      painter: "spell",
      entryPart: "TRANSLATION",
    }));
  }
}
