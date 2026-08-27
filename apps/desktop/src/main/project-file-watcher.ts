// SPDX-License-Identifier: GPL-3.0-or-later

import {
  readdirSync,
  watch as watchDirectory,
  type FSWatcher,
  type WatchEventType,
} from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";

export type ExternalProjectChange = {
  root: string;
  paths: string[];
};

type WatchHandle = Pick<FSWatcher, "close">;
type WatchFactory = (
  path: string,
  listener: (eventType: WatchEventType, filename: string | Buffer | null) => void,
) => WatchHandle;

const WATCHED_PROJECT_DIRS = ["source", "omegat", "tm", "glossary", "dictionary"];

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
  private readonly watchers: WatchHandle[] = [];
  private readonly changed = new Set<string>();
  private timer: NodeJS.Timeout | null = null;

  constructor(
    private readonly publish: (event: ExternalProjectChange) => void,
    private readonly debounceMs = 120,
    private readonly watchFactory: WatchFactory = (path, listener) =>
      watchDirectory(path, listener),
  ) {}

  watch(root: string): void {
    this.close();
    this.root = resolve(root);
    const directories = [this.root];
    WATCHED_PROJECT_DIRS.forEach((name) =>
      collectDirectories(join(this.root!, name), directories)
    );
    for (const directory of new Set(directories)) {
      try {
        this.watchers.push(this.watchFactory(directory, (_eventType, filename) => {
          if (!this.root || filename === null) return;
          const raw = filename.toString();
          const path = isAbsolute(raw) ? raw : join(directory, raw);
          if (!this.isProjectInput(path)) return;
          this.changed.add(resolve(path));
          this.schedule();
        }));
      } catch {
        // Missing/inaccessible optional project directories are not fatal.
      }
    }
  }

  close(): void {
    this.watchers.splice(0).forEach((watcher) => watcher.close());
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.changed.clear();
    this.root = null;
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
      this.changed.clear();
      this.timer = null;
      if (root && paths.length > 0) this.publish({ root, paths });
    }, this.debounceMs);
  }
}
