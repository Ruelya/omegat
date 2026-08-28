// SPDX-License-Identifier: GPL-3.0-or-later

export type ProjectEventKind =
  | "load"
  | "create"
  | "close"
  | "reload"
  | "entry"
  | "external-refresh";

export type ProjectEvent = {
  serial: number;
  kind: ProjectEventKind;
  projectGeneration: number;
  entryGeneration: number;
  projectId: string | null;
  entryKey: string | null;
  changedEntryKeys: string[];
};

export type ProjectEventListener = (event: ProjectEvent) => void;

function copyEvent(event: ProjectEvent): ProjectEvent {
  return {
    ...event,
    changedEntryKeys: [...event.changedEntryKeys],
  };
}

/**
 * Renderer equivalent of OmegaT's CoreEvents project/entry boundary.
 *
 * Project events invalidate every entry-scoped consumer before asynchronous
 * project work starts. Entry and external-refresh events invalidate the
 * current pane work even when the next project exposes the same EntryKey.
 */
export class ProjectEventBus {
  private event: ProjectEvent = {
    serial: 0,
    kind: "close",
    projectGeneration: 0,
    entryGeneration: 0,
    projectId: null,
    entryKey: null,
    changedEntryKeys: [],
  };

  private readonly listeners = new Set<ProjectEventListener>();

  current(): ProjectEvent {
    return copyEvent(this.event);
  }

  subscribe(listener: ProjectEventListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  publishProject(
    kind: Extract<ProjectEventKind, "load" | "create" | "close" | "reload">,
    projectId: string | null,
  ): ProjectEvent {
    return this.publish({
      kind,
      projectGeneration: this.event.projectGeneration + 1,
      entryGeneration: this.event.entryGeneration + 1,
      projectId,
      entryKey: null,
      changedEntryKeys: [],
    });
  }

  publishEntry(projectId: string | null, entryKey: string): ProjectEvent {
    const projectChanged = projectId !== this.event.projectId;
    return this.publish({
      kind: "entry",
      projectGeneration: this.event.projectGeneration + (projectChanged ? 1 : 0),
      entryGeneration: this.event.entryGeneration + 1,
      projectId,
      entryKey,
      changedEntryKeys: [],
    });
  }

  publishExternalRefresh(
    projectId: string | null,
    entryKey: string | null,
    changedEntryKeys: readonly string[] = [],
  ): ProjectEvent {
    const projectChanged = projectId !== this.event.projectId;
    return this.publish({
      kind: "external-refresh",
      projectGeneration: this.event.projectGeneration + (projectChanged ? 1 : 0),
      entryGeneration: this.event.entryGeneration + 1,
      projectId,
      entryKey,
      changedEntryKeys: [...changedEntryKeys],
    });
  }

  /** Test/reset boundary; listeners stay attached. */
  reset(): ProjectEvent {
    this.event = {
      serial: 0,
      kind: "close",
      projectGeneration: 0,
      entryGeneration: 0,
      projectId: null,
      entryKey: null,
      changedEntryKeys: [],
    };
    const snapshot = this.current();
    this.listeners.forEach((listener) => listener(snapshot));
    return snapshot;
  }

  private publish(next: Omit<ProjectEvent, "serial">): ProjectEvent {
    this.event = {
      ...next,
      serial: this.event.serial + 1,
      changedEntryKeys: [...next.changedEntryKeys],
    };
    const snapshot = this.current();
    this.listeners.forEach((listener) => listener(snapshot));
    return snapshot;
  }
}

export const projectEvents = new ProjectEventBus();
