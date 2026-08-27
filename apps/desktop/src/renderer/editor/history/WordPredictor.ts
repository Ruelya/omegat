/** Java `org.omegat.gui.editor.history.WordPredictor`. */
export type Prediction = { word: string; frequency: number };
export type PredictorModel = Map<string, Map<string, number>>;

export const MIN_FREQUENCY = 10;

export function emptyPredictor(): PredictorModel {
  return new Map();
}

export function trainPredictorTokens(model: PredictorModel, tokens: string[] | null): PredictorModel {
  if (tokens == null) throw new TypeError("Prediction seed can't be null");
  if (tokens.length === 0) return model;
  for (let i = 0; i < tokens.length - 1; i++) {
    const a = tokens[i]!;
    const b = tokens[i + 1]!;
    if (!model.has(a)) model.set(a, new Map());
    const inner = model.get(a)!;
    inner.set(b, (inner.get(b) ?? 0) + 1);
  }
  return model;
}

export function trainPredictor(translations: string[]): PredictorModel {
  const model = emptyPredictor();
  for (const text of translations) {
    const words = text.split(/[^\p{L}\p{N}']+/u).filter(Boolean);
    trainPredictorTokens(model, words);
  }
  return model;
}

export function predictWord(model: PredictorModel, seed: string | null): Prediction[] {
  if (seed == null) throw new TypeError("Prediction seed can't be null");
  if (model.size === 0 || seed.length === 0) return [];
  const nexts = model.get(seed);
  if (!nexts) return [];
  const entries = [...nexts.entries()].filter(([, n]) => n > 1);
  const total = entries.reduce((a, [, n]) => a + n, 0);
  return entries
    .map(([word, n]) => ({ word, frequency: (n / total) * 100 }))
    .filter((p) => p.frequency >= MIN_FREQUENCY)
    .sort((a, b) => b.frequency - a.frequency || a.word.localeCompare(b.word));
}

export function predictNext(model: PredictorModel, prev: string): { word: string; pct: number }[] {
  const trailing = /\s$/.test(prev);
  const tokens = prev.trim().split(/\s+/).filter(Boolean);
  const seed = trailing ? tokens.at(-1) ?? "" : tokens.at(-2) ?? "";
  if (!seed) return [];
  return predictWord(model, seed).map((p) => ({ word: p.word, pct: Math.round(p.frequency) }));
}

export class WordPredictor {
  data: PredictorModel = emptyPredictor();
  reset() {
    this.data = emptyPredictor();
  }
  train(tokens: string[] | null) {
    trainPredictorTokens(this.data, tokens);
  }
  predictWord(seed: string | null): Prediction[] {
    return predictWord(this.data, seed);
  }
}
