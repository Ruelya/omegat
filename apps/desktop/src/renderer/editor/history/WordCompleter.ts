/** Java `org.omegat.gui.editor.history.WordCompleter`. */
export const MIN_CHARS = 3;

export class WordCompleter {
  data: string[] = [];

  reset() {
    this.data = [];
  }

  train(tokens: string[] | null) {
    if (tokens == null) throw new TypeError("Should throw NPE when given null input");
    for (const token of tokens) {
      if ([...token].length > MIN_CHARS && !this.data.includes(token)) this.data.push(token);
    }
  }

  completeWord(seed: string | null): string[] {
    if (seed == null) throw new TypeError("Should throw NPE when given null input");
    if (this.data.length === 0 || [...seed].length < MIN_CHARS) return [];
    return this.data.filter((s) => s.startsWith(seed) && s.toLowerCase() !== seed.toLowerCase());
  }
}

export function completeWords(translations: string[], prefix: string): string[] {
  const c = new WordCompleter();
  for (const text of translations) {
    c.train(text.split(/[^\p{L}\p{N}']+/u).filter(Boolean));
  }
  return c.completeWord(prefix);
}
