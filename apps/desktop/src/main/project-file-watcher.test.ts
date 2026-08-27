import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProjectFileWatcher } from "./project-file-watcher";

const roots: string[] = [];

afterEach(() => {
  roots.splice(0).forEach((root) => rmSync(root, { recursive: true, force: true }));
});

describe("ProjectFileWatcher", () => {
  it("coalesces real project input paths and ignores unrelated root files", async () => {
    const root = mkdtempSync(join(tmpdir(), "omegat-watch-"));
    roots.push(root);
    mkdirSync(join(root, "source", "nested"), { recursive: true });
    mkdirSync(join(root, "omegat"), { recursive: true });
    const listeners = new Map<string, (event: "change", filename: string) => void>();
    const closed: string[] = [];
    const publish = vi.fn();
    const watcher = new ProjectFileWatcher(
      publish,
      0,
      (path, listener) => {
        listeners.set(resolve(path), listener);
        return { close: () => closed.push(resolve(path)) };
      },
    );

    watcher.watch(root);
    listeners.get(resolve(root))?.("change", "notes.txt");
    listeners.get(resolve(root))?.("change", "omegat.project");
    listeners.get(resolve(root, "source", "nested"))?.("change", "chapter.txt");
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));

    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      paths: [
        resolve(root, "omegat.project"),
        resolve(root, "source", "nested", "chapter.txt"),
      ],
    });
    watcher.close();
    expect(closed.sort()).toEqual([...listeners.keys()].sort());
  });

  it("installs watchers for runtime directories and coalesces sidecar events", async () => {
    const root = mkdtempSync(join(tmpdir(), "omegat-watch-runtime-"));
    roots.push(root);
    const listeners = new Map<string, (event: "change" | "rename", filename: string) => void>();
    const publish = vi.fn();
    const watcher = new ProjectFileWatcher(
      publish,
      0,
      (path, listener) => {
        listeners.set(resolve(path), listener);
        return { close: () => undefined };
      },
    );

    watcher.watch(root);
    expect([...listeners.keys()]).toEqual([resolve(root)]);
    mkdirSync(join(root, "source", "runtime", "deeper"), { recursive: true });
    listeners.get(resolve(root))?.("rename", "source");
    expect([...listeners.keys()].sort()).toEqual([
      resolve(root),
      resolve(root, "source"),
      resolve(root, "source", "runtime"),
      resolve(root, "source", "runtime", "deeper"),
    ].sort());

    listeners
      .get(resolve(root, "source", "runtime", "deeper"))
      ?.("change", "created.txt");
    watcher.acceptExternalChange({
      root,
      paths: [
        "source/runtime/deeper/created.txt",
        "unrelated.txt",
      ],
    });
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));

    expect(publish).toHaveBeenCalledTimes(1);
    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      paths: [
        resolve(root, "source"),
        resolve(root, "source", "runtime", "deeper", "created.txt"),
      ],
    });
    watcher.close();
  });
});
