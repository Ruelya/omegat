import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { getWordBoundary } from "./EditorUtils";
import { WordCompleter } from "./history/WordCompleter";
import { WordPredictor } from "./history/WordPredictor";

const goldDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../../fixtures/goldens/editor");

function load(name: string) {
  return JSON.parse(readFileSync(join(goldDir, name), "utf8"));
}

describe("WordPredictor / WordCompleter / EditorUtils Java goldens", () => {
  it("WordPredictorTest#testWordPredictor assert_eq", () => {
    const g = load("WordPredictorTest#testWordPredictor.json");
    const p = new WordPredictor();
    expect(p.predictWord("a")).toEqual([]);
    for (const tokens of g.train) p.train(tokens);
    for (const [seed, expected] of Object.entries(g.predict) as [string, { word: string; frequency: number }[]][]) {
      expect(p.predictWord(seed), seed).toEqual(expected);
    }
  });

  it("WordPredictorTest#testWordFrequency assert_eq", () => {
    const g = load("WordPredictorTest#testWordFrequency.json");
    const p = new WordPredictor();
    for (const step of g.steps) {
      for (const tokens of step.train) p.train(tokens);
      expect(p.predictWord("big")).toEqual(step.predict_big);
    }
  });

  it("WordPredictorTest#testMinFrequency / reset / empty", () => {
    const p = new WordPredictor();
    const words = [
      "bear",
      "bench",
      "bazooka",
      "bazinga",
      "balloon",
      "boulder",
      "blanket",
      "balcony",
      "binder",
      "book",
    ];
    for (let i = 0; i < 2; i++) {
      for (const w of words) p.train(["a", "big", "brown", w]);
    }
    const brown = p.predictWord("brown");
    expect(brown).toHaveLength(10);
    expect(brown[0]!.word).toBe("balcony");
    expect(brown[9]!.word).toBe("boulder");
    p.train(["a", "big", "brown", "bath"]);
    p.train(["a", "big", "brown", "bath"]);
    expect(p.predictWord("brown")).toEqual([]);
    expect(p.predictWord("a")).toEqual([{ word: "big", frequency: 100 }]);
    p.reset();
    expect(p.predictWord("a")).toEqual([]);
    expect(() => p.train(null)).toThrow();
    expect(p.predictWord("")).toEqual([]);
    expect(() => p.predictWord(null)).toThrow();
  });

  it("WordCompleterTest methods assert_eq", () => {
    const g = load("WordCompleterTest#testWordCompletion.json");
    const c = new WordCompleter();
    expect(c.completeWord("foo")).toEqual([]);
    c.train(g.train);
    for (const [seed, expected] of Object.entries(g.complete) as [string, string[]][]) {
      expect(c.completeWord(seed), seed).toEqual(expected);
    }
    c.reset();
    expect(c.completeWord("foo")).toEqual([]);
    expect(() => c.train(null)).toThrow();
    expect(c.completeWord("")).toEqual([]);
    expect(() => c.completeWord(null)).toThrow();
  });

  it("EditorUtilsTest word-boundary goldens assert_eq", () => {
    for (const name of [
      "EditorUtilsTest#testGetBoundarySimple.json",
      "EditorUtilsTest#testGetWordBoundaryJa.json",
      "EditorUtilsTest#testGetWordBoundaryCn.json",
    ]) {
      const g = load(name);
      for (const c of g.cases) {
        expect(getWordBoundary(g.locale, g.text, c.offset, c.forward), `${name} ${c.offset} ${c.forward}`).toBe(
          c.expect,
        );
      }
    }
  });
});
