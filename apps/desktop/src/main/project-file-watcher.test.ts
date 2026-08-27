import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
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

    watcher.watch(root, 7);
    listeners.get(resolve(root))?.("change", "notes.txt");
    listeners.get(resolve(root))?.("change", "omegat.project");
    listeners.get(resolve(root, "source", "nested"))?.("change", "chapter.txt");
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));

    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      generation: 7,
      paths: [
        resolve(root, "omegat.project"),
        resolve(root, "source", "nested", "chapter.txt"),
      ],
      sources: ["native"],
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

    watcher.watch(root, 11);
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
      generation: 11,
      paths: [
        resolve(root, "source"),
        resolve(root, "source", "runtime", "deeper", "created.txt"),
      ],
      sources: ["native", "sidecar"],
    });
    watcher.close();
  });

  it("suppresses active write sources and advances same-root generations", async () => {
    const root = mkdtempSync(join(tmpdir(), "omegat-watch-generation-"));
    roots.push(root);
    mkdirSync(join(root, "omegat"), { recursive: true });
    const listeners = new Map<string, (event: "change", filename: string) => void>();
    const publish = vi.fn();
    const watcher = new ProjectFileWatcher(
      publish,
      0,
      (path, listener) => {
        listeners.set(resolve(path), listener);
        return { close: () => undefined };
      },
    );

    watcher.watch(root, 20);
    const endWrite = watcher.beginWriteSource("project.save");
    listeners.get(resolve(root, "omegat"))?.("change", "project_save.tmx");
    watcher.acceptExternalChange({
      root,
      paths: ["omegat/project_save.tmx"],
    });
    endWrite();
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));
    expect(publish).not.toHaveBeenCalled();

    watcher.watch(root, 21);
    listeners.get(resolve(root, "omegat"))?.("change", "project_save.tmx");
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));
    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      generation: 21,
      paths: [resolve(root, "omegat", "project_save.tmx")],
      sources: ["native"],
    });
    watcher.close();
  });

  it("preserves delayed self-write suppression across same-root generation changes", async () => {
    const root = mkdtempSync(join(tmpdir(), "omegat-watch-rebind-"));
    roots.push(root);
    mkdirSync(join(root, "source"), { recursive: true });
    mkdirSync(join(root, "omegat"), { recursive: true });
    const listeners = new Map<string, (event: "change", filename: string) => void>();
    const publish = vi.fn();
    const watcher = new ProjectFileWatcher(
      publish,
      0,
      (path, listener) => {
        listeners.set(resolve(path), listener);
        return { close: () => undefined };
      },
    );

    watcher.watch(root, 40);
    const endWrite = watcher.beginWriteSource("project.save");
    const saveTmx = join(root, "omegat", "project_save.tmx");
    writeFileSync(saveTmx, "sidecar save");
    listeners.get(resolve(root, "omegat"))?.("change", "project_save.tmx");
    endWrite();

    watcher.watch(root, 41);
    listeners.get(resolve(root, "omegat"))?.("change", "project_save.tmx");
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));
    expect(publish).not.toHaveBeenCalled();

    const source = join(root, "source", "external.txt");
    writeFileSync(source, "external");
    listeners.get(resolve(root, "source"))?.("change", "external.txt");
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 5));
    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      generation: 41,
      paths: [resolve(source)],
      sources: ["native"],
    });
    watcher.close();
  });

  it("suppresses delayed real fs echoes but publishes the next external write", async () => {
    const root = mkdtempSync(join(tmpdir(), "omegat-watch-real-write-"));
    roots.push(root);
    mkdirSync(join(root, "source"), { recursive: true });
    mkdirSync(join(root, "omegat"), { recursive: true });
    const source = join(root, "source", "chapter.txt");
    writeFileSync(source, "initial");
    const publish = vi.fn();
    const watcher = new ProjectFileWatcher(publish, 10);
    watcher.watch(root, 33);

    const endWrite = watcher.beginWriteSource("project.save");
    writeFileSync(join(root, "omegat", "project_save.tmx"), "sidecar save");
    endWrite();
    await new Promise((resolveTimer) => setTimeout(resolveTimer, 100));
    expect(publish).not.toHaveBeenCalled();

    writeFileSync(source, "real external change with a distinct fingerprint");
    for (let attempts = 0; attempts < 50 && publish.mock.calls.length === 0; attempts += 1) {
      await new Promise((resolveTimer) => setTimeout(resolveTimer, 10));
    }
    expect(publish).toHaveBeenCalledTimes(1);
    expect(publish).toHaveBeenCalledWith({
      root: resolve(root),
      generation: 33,
      paths: [resolve(source)],
      sources: ["native"],
    });
    watcher.close();
  });
});
