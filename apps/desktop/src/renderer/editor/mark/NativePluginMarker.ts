/** Executable cdylib Marker bridge: renderer -> NDJSON sidecar -> plugin ABI. */
import type { EditorController } from "../EditorController";
import type { PluginMarkerInfoDto } from "../../lib/types";
import type { IAsyncMarker, MarkerInput } from "./IMarker";
import type { EntryPart, Mark } from "./Mark";

export type MarkerRpc = (method: string, params?: unknown) => Promise<unknown>;

type NativeMarkDto = {
  start_offset: number;
  end_offset: number;
  painter: string;
  painter_color?: string;
  tooltip_text?: string;
  entry_part: EntryPart;
};

type NativeMarksResult = {
  marks: NativeMarkDto[];
};

async function sidecarRpc(method: string, params?: unknown): Promise<unknown> {
  if (typeof window === "undefined" || !window.omegat) {
    throw new Error("sidecar bridge unavailable");
  }
  return window.omegat.rpc(method, params);
}

function toMark(dto: NativeMarkDto, input: MarkerInput): Mark {
  const limit = dto.entry_part === "SOURCE"
    ? input.sourceText?.length ?? 0
    : dto.entry_part === "TRANSLATION"
      ? input.translationText?.length ?? 0
      : -1;
  if (
    !Number.isInteger(dto.start_offset)
    || !Number.isInteger(dto.end_offset)
    || dto.start_offset < 0
    || dto.end_offset <= dto.start_offset
    || dto.end_offset > limit
    || !dto.painter?.trim()
  ) {
    throw new Error(`invalid native marker span ${dto.start_offset}..${dto.end_offset}`);
  }
  return {
    startOffset: dto.start_offset,
    endOffset: dto.end_offset,
    painter: dto.painter,
    painterColor: dto.painter_color,
    toolTipText: dto.tooltip_text,
    entryPart: dto.entry_part,
  };
}

export class NativePluginMarker implements IAsyncMarker {
  constructor(
    readonly info: PluginMarkerInfoDto,
    private readonly rpc: MarkerRpc = sidecarRpc,
  ) {}

  async getMarksForEntryAsync(input: MarkerInput): Promise<Mark[] | null> {
    const result = await this.rpc("markers.query", {
      id: this.info.id,
      entry_key: input.entryKey ?? null,
      source_text: input.sourceText,
      translation_text: input.translationText,
      is_active: input.isActive,
      display_source: input.displaySource ?? false,
      is_alt: input.isAlt ?? false,
      from_auto: input.fromAuto ?? false,
      from_mt: input.fromMt ?? false,
      linked: input.linked ?? null,
      enabled: input.enabled ?? true,
      protected_parts: input.protectedParts ?? [],
    }) as NativeMarksResult;
    if (!result || !Array.isArray(result.marks)) {
      throw new Error(`native marker ${this.info.id} returned no marks array`);
    }
    return result.marks.map((mark) => toMark(mark, input));
  }
}

export type NativePluginMarkerConnection = {
  ready: Promise<boolean>;
  release: () => void;
};

/**
 * Reference-counted lifecycle for React StrictMode. Stale `markers.list`
 * responses cannot register providers after an unmount or a newer connection.
 */
export class NativePluginMarkerBridge {
  private clients = 0;
  private request = 0;
  private installedNames: string[] = [];

  constructor(
    private readonly controller: EditorController,
    private readonly rpc: MarkerRpc = sidecarRpc,
  ) {}

  connect(): NativePluginMarkerConnection {
    this.clients += 1;
    const token = ++this.request;
    let released = false;
    const ready = this.install(token);
    return {
      ready,
      release: () => {
        if (released) return;
        released = true;
        this.clients = Math.max(0, this.clients - 1);
        if (this.clients === 0) {
          this.request += 1;
          this.uninstall();
        }
      },
    };
  }

  private async install(token: number): Promise<boolean> {
    const infos = await this.rpc("markers.list", {}) as PluginMarkerInfoDto[];
    if (token !== this.request || this.clients === 0) return false;
    if (!Array.isArray(infos)) throw new Error("markers.list returned no array");
    this.uninstall();
    const installed: string[] = [];
    try {
      for (const info of infos) {
        if (!info.id?.trim() || !info.name?.trim()) {
          throw new Error("native marker id and name are required");
        }
        this.controller.registerPluginMarker(
          info.name,
          new NativePluginMarker(info, this.rpc),
        );
        installed.push(info.name);
      }
      this.installedNames = installed;
      return true;
    } catch (error) {
      for (const name of installed.reverse()) {
        this.controller.unregisterPluginMarker(name);
      }
      throw error;
    }
  }

  private uninstall(): void {
    for (const name of this.installedNames.splice(0).reverse()) {
      this.controller.unregisterPluginMarker(name);
    }
  }
}
