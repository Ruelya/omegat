/** Java `org.omegat.gui.editor.history.WordPredictor` — next-word model. */
export type PredictorModel = Map<string, Map<string, number>>;

export function trainPredictor(translations: string[]): PredictorModel {
  const model: PredictorModel = new Map();
  for (const text of translations) {
    const words = text.split(/[^\p{L}\p{N}']+/u).filter(Boolean);
    for (let i = 0; i < words.length - 1; i++) {
      const a = words[i]!.toLowerCase();
      const b = words[i + 1]!;
      if (!model.has(a)) model.set(a, new Map());
      const inner = model.get(a)!;
      inner.set(b, (inner.get(b) ?? 0) + 1);
    }
  }
  return model;
}

export function predictNext(model: PredictorModel, prev: string): { word: string; pct: number }[] {
  const trailing = /\s$/.test(prev);
  const tokens = prev.trim().split(/\s+/).filter(Boolean);
  const seed = trailing ? tokens.at(-1) ?? "" : tokens.at(-2) ?? "";
  const ctx = trailing ? "" : tokens.at(-1) ?? "";
  if (!seed) return [];
  const nexts = model.get(seed.toLowerCase());
  if (!nexts) return [];
  const total = [...nexts.values()].reduce((a, b) => a + b, 0);
  return [...nexts.entries()]
    .filter(([w]) => !ctx || (w.startsWith(ctx) && w !== ctx))
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([word, n]) => ({ word, pct: Math.round((n / total) * 100) }));
}
