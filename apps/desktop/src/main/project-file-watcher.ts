// SPDX-License-Identifier: GPL-3.0-or-later

import {
  readdirSync,
  statSync,
  watch as watchDirectory,
  type FSWatcher,
  type WatchEventType,
} from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";

export type ExternalProjectChange = {
  root: string;
  paths: string[];
  fingerprints: Record<string, string | null>;
  generation: number;
  sources: ProjectChangeSource[];
};

export type ProjectChangeSource = "native" | "sidecar";

type SidecarProjectChange = Pick<ExternalProjectChange, "root" | "paths">;

type WatchHandle = Pick<FSWatcher, "close">;
type WatchFactory = (
  path: string,
  listener: (eventType: WatchEventType, filename: string | Buffer | null) => void,
) => WatchHandle;

const WATCHED_PROJECT_DIRS = ["source", "omegat", "tm", "glossary", "dictionary"];

function fileFingerprint(path: string): string | null {
  try {
    const stats = statSync(path, { bigint: true });
    if (!stats.isFile()) return null;
    return [
      stats.dev,
      stats.ino,
      stats.size,
      stats.mtimeNs,
      stats.ctimeNs,
    ].join(":");
  } catch {
    return null;
  }
}

function collectProjectFingerprints(root: string): Map<string, string | null> {
  const files = new Map<string, string | null>();
  const record = (path: string) => {
    const fingerprint = fileFingerprint(path);
    if (fingerprint !== null) files.set(resolve(path), fingerprint);
  };
  const collect = (path: string) => {
    let entries;
    try {
      entries = readdirSync(path, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue;
      const child = join(path, entry.name);
      if (entry.isDirectory()) collect(child);
      else if (entry.isFile()) record(child);
    }
  };
  record(join(root, "omegat.project"));
  WATCHED_PROJECT_DIRS.forEach((directory) => collect(join(root, directory)));
  return files;
}

function collectDirectories(path: string, directories: string[]): void {
  let entries;
  try {
    entries = readdirSync(path, { withFileTypes: true });
  } catch {
    return;
  }
  directories.push(path);
  for (const entry of entries) {
    if (entry.isDirectory() && !entry.isSymbolicLink()) {
      collectDirectories(join(path, entry.name), directories);
    }
  }
}

/**
 * Native filesystem boundary for source/team-owned project files.
 *
 * Node's recursive watch is unavailable on Linux, so every existing project
 * subdirectory receives its own watcher. Events are coalesced before they are
 * forwarded to the renderer's EXTERNAL_REFRESH bus.
 */
export class ProjectFileWatcher {
  private root: string | null = null;
  private generation = 0;
  private readonly watchers = new Map<string, WatchHandle>();
  private readonly changed = new Set<string>();
  private readonly sources = new Set<ProjectChangeSource>();
  private readonly activeWriteSources = new Map<number, string>();
  private readonly suppressedNativeFingerprints = new Map<string, string | null>();
  private writeBaseline: Map<string, string | null> | null = null;
  private nextWriteSource = 1;
  private timer: NodeJS.Timeout | null = null;

  constructor(
    private readonly publish: (event: ExternalProjectChange) => void,
    private readonly debounceMs = 120,
    private readonly watchFactory: WatchFactory = (path, listener) =>
      watchDirectory(path, listener),
  ) {}

  watch(root: string, generation = this.generation + 1): number {
    const nextRoot = resolve(root);
    if (this.root === nextRoot) {
      if (this.timer) clearTimeout(this.timer);
      this.timer = null;
      this.changed.clear();
      this.sources.clear();
      this.generation = generation;
      this.refreshDirectoryWatches();
      return this.generation;
    }
    this.close();
    this.root = nextRoot;
    this.generation = generation;
    this.refreshDirectoryWatches();
    return this.generation;
  }

  /**
   * Merge a sidecar-originated filesystem notification into the same debounce
   * window as native `fs.watch` events.
   */
  acceptExternalChange(event: SidecarProjectChange): void {
    if (!this.root || resolve(event.root) !== this.root) return;
    this.refreshDirectoryWatches();
    for (const raw of event.paths) {
      const path = resolve(isAbsolute(raw) ? raw : join(this.root, raw));
      this.recordChange(path, "sidecar");
    }
    if (this.changed.size > 0) this.schedule();
  }

  /**
   * Suppress native watcher echoes while a sidecar operation writes project
   * inputs. The sidecar scanner has an equivalent begin/end boundary.
   */
  beginWriteSource(source: string): () => void {
    const token = this.nextWriteSource++;
    if (this.activeWriteSources.size === 0 && this.root) {
      this.writeBaseline = collectProjectFingerprints(this.root);
    }
    this.activeWriteSources.set(token, source);
    return () => {
      if (!this.activeWriteSources.delete(token)) return;
      if (this.activeWriteSources.size > 0 || !this.root) return;
      const before = this.writeBaseline ?? new Map<string, string | null>();
      const after = collectProjectFingerprints(this.root);
      const paths = new Set([...before.keys(), ...after.keys()]);
      for (const path of paths) {
        const previous = before.get(path) ?? null;
        const current = after.get(path) ?? null;
        if (previous !== current) {
          this.suppressedNativeFingerprints.set(path, current);
        }
      }
      this.writeBaseline = null;
    };
  }

  private refreshDirectoryWatches(): void {
    if (!this.root) return;
    const directories = [this.root];
    WATCHED_PROJECT_DIRS.forEach((name) =>
      collectDirectories(join(this.root!, name), directories)
    );
    const desired = new Set(directories.map((directory) => resolve(directory)));
    for (const [directory, watcher] of this.watchers) {
      if (desired.has(directory)) continue;
      watcher.close();
      this.watchers.delete(directory);
    }
    for (const directory of desired) {
      if (this.watchers.has(directory)) continue;
      try {
        const watcher = this.watchFactory(directory, (eventType, filename) => {
          if (!this.root || filename === null) return;
          const raw = filename.toString();
          const path = isAbsolute(raw) ? raw : join(directory, raw);
          this.recordChange(path, "native");
          if (eventType === "rename") this.refreshDirectoryWatches();
          if (this.changed.size > 0) this.schedule();
        });
        this.watchers.set(directory, watcher);
      } catch {
        // Missing/inaccessible optional project directories are not fatal.
      }
    }
  }

  close(): void {
    this.watchers.forEach((watcher) => watcher.close());
    this.watchers.clear();
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.changed.clear();
    this.sources.clear();
    this.activeWriteSources.clear();
    this.suppressedNativeFingerprints.clear();
    this.writeBaseline = null;
    this.root = null;
    this.generation += 1;
  }

  private isProjectInput(path: string): boolean {
    if (!this.root) return false;
    const rel = relative(this.root, resolve(path)).replaceAll("\\", "/");
    if (!rel || rel.startsWith("../")) return false;
    return rel === "omegat.project"
      || WATCHED_PROJECT_DIRS.some((directory) =>
        rel === directory || rel.startsWith(`${directory}/`)
      );
  }

  private schedule(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      const root = this.root;
      const paths = [...this.changed].sort();
      const sources = [...this.sources].sort();
      const generation = this.generation;
      this.changed.clear();
      this.sources.clear();
      this.timer = null;
      if (root && paths.length > 0) {
        this.publish({
          root,
          paths,
          fingerprints: Object.fromEntries(
            paths.map((path) => [path, fileFingerprint(path)]),
          ),
          generation,
          sources,
        });
      }
    }, this.debounceMs);
  }

  private recordChange(path: string, source: ProjectChangeSource): void {
    const resolvedPath = resolve(path);
    if (!this.isProjectInput(resolvedPath) || this.activeWriteSources.size > 0) return;
    if (
      source === "native"
      && this.suppressedNativeFingerprints.has(resolvedPath)
    ) {
      const expected = this.suppressedNativeFingerprints.get(resolvedPath) ?? null;
      if (fileFingerprint(resolvedPath) === expected) return;
      this.suppressedNativeFingerprints.delete(resolvedPath);
    }
    this.changed.add(resolvedPath);
    this.sources.add(source);
  }
}
