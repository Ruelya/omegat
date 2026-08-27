import { describe, expect, it } from "vitest";
import {
  DEFAULT_DOCK_LAYOUT,
  layoutFromPrefs,
  layoutToPrefs,
  normalizeDockLayout,
  parseDockLayout,
} from "./layout";
import { defaultDocking } from "./preferences";

describe("dock layout", () => {
  it("matches DockingDefaults.xml ratios", () => {
    expect(DEFAULT_DOCK_LAYOUT.left).toBe(0.25);
    expect(DEFAULT_DOCK_LAYOUT.notes).toBe(0.2);
    expect(DEFAULT_DOCK_LAYOUT.editorStack).toBe(0.65);
    expect(DEFAULT_DOCK_LAYOUT.editorMain).toBe(0.75);
    expect(DEFAULT_DOCK_LAYOUT.props).toBe(0.5);
    expect(DEFAULT_DOCK_LAYOUT.matches).toBe(0.8);
  });

  it("persists and restores from typed prefs.docking_layout", () => {
    const next = normalizeDockLayout({ left: 0.33, matches: 0.55, showMt: false });
    const saved = layoutToPrefs(next);
    const restored = layoutFromPrefs(saved);
    expect(restored.left).toBeCloseTo(0.33);
    expect(restored.matches).toBeCloseTo(0.55);
    expect(restored.showMt).toBe(false);
    expect(layoutFromPrefs(defaultDocking()).east).toBeCloseTo(0.78);
    expect(parseDockLayout("not-json")).toEqual(DEFAULT_DOCK_LAYOUT);
  });
});
