import { describe, expect, it } from "vitest";
import { EditorController } from "../EditorController";
import type { EntryKeyDto } from "../../lib/types";
import {
  NativePluginMarkerBridge,
  type MarkerRpc,
} from "./NativePluginMarker";

const info = {
  plugin_id: "example",
  id: "example.native-marker",
  name: "org.omegat.example.NativePluginMarker",
};

const entryKey: EntryKeyDto = {
  file: "source/sample.example",
  source_text: "Hello from plugin",
  id: "0",
  prev: "",
  next: "Second line",
  path: null,
};

describe("native cdylib Marker product bridge", () => {
  it("passes the complete EntryKey and applies UTF-16 spans to Document3", async () => {
    const calls: [string, unknown][] = [];
    const rpc: MarkerRpc = async (method, params) => {
      calls.push([method, params]);
      if (method === "markers.list") return [info];
      if (method === "markers.query") {
        return {
          marks: [{
            start_offset: 3,
            end_offset: 9,
            painter: "native-plugin",
            painter_color: "#7c3aed",
            tooltip_text: "Example marker in source/sample.example",
            entry_part: "TRANSLATION",
          }],
        };
      }
      throw new Error(`unexpected method ${method}`);
    };
    const controller = new EditorController();
    controller.loadProject([{
      key: entryKey,
      file: entryKey.file,
      source: entryKey.source_text,
      translation: "😀 plugin",
      id: "0",
    }]);
    const bridge = new NativePluginMarkerBridge(controller, rpc);
    const connection = bridge.connect();

    expect(await connection.ready).toBe(true);
    expect(await controller.refreshCurrentMarkersAsync()).toBe(true);
    expect(calls).toEqual([
      ["markers.list", {}],
      ["markers.query", {
        id: "example.native-marker",
        entry_key: entryKey,
        source_text: "Hello from plugin",
        translation_text: "😀 plugin",
        is_active: true,
        display_source: false,
        is_alt: false,
        from_auto: false,
        from_mt: false,
        linked: null,
        enabled: true,
        protected_parts: [],
      }],
    ]);
    expect(
      controller.markerSnapshot?.marks.filter(({ painter }) =>
        painter === "native-plugin"
      ),
    ).toEqual([{
      startOffset: 3,
      endOffset: 9,
      painter: "native-plugin",
      painterColor: "#7c3aed",
      toolTipText: "Example marker in source/sample.example",
      entryPart: "TRANSLATION",
    }]);
    expect(
      controller.getOmDocument()?.spans.filter(({ style }) =>
        style.startsWith("marker:native-plugin")
      ),
    ).toEqual([{
      start: controller.getOmDocument()!.translationStart + 3,
      end: controller.getOmDocument()!.translationStart + 9,
      style: "marker:native-plugin:#7c3aed",
    }]);

    connection.release();
    expect(controller.markers.getMarkerNames()).not.toContain(info.name);
    expect(
      controller.getOmDocument()?.spans.some(({ style }) =>
        style.startsWith("marker:native-plugin")
      ),
    ).toBe(false);
  });

  it("discards a StrictMode-era marker list after release", async () => {
    let resolveFirst!: (value: unknown) => void;
    let lists = 0;
    const rpc: MarkerRpc = (method) => {
      if (method !== "markers.list") throw new Error("query was not expected");
      lists += 1;
      if (lists === 1) {
        return new Promise((resolve) => {
          resolveFirst = resolve;
        });
      }
      return Promise.resolve([info]);
    };
    const controller = new EditorController();
    const bridge = new NativePluginMarkerBridge(controller, rpc);
    const stale = bridge.connect();
    stale.release();
    const current = bridge.connect();

    expect(await current.ready).toBe(true);
    resolveFirst([{
      ...info,
      id: "stale.marker",
      name: "org.omegat.example.StaleMarker",
    }]);
    expect(await stale.ready).toBe(false);
    expect(controller.markers.getMarkerNames().filter((name) =>
      name.startsWith("org.omegat.example.")
    )).toEqual([info.name]);

    current.release();
    expect(controller.markers.getMarkerNames()).not.toContain(info.name);
  });
});
