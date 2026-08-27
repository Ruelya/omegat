import { describe, expect, it, vi } from "vitest";
import {
  createApplicationLifecycle,
  registerApplicationLifecycle,
  type LifecycleApp,
  type LifecycleIpc,
} from "./lifecycle";

describe("packaged application lifecycle", () => {
  it("registers quit and relaunch on the main-process IPC boundary", () => {
    const handlers = new Map<string, () => void>();
    const ipc: LifecycleIpc = {
      handle: vi.fn((channel, handler) => {
        handlers.set(channel, handler);
      }),
    };
    const app: LifecycleApp = {
      relaunch: vi.fn(),
      quit: vi.fn(),
      exit: vi.fn(),
    };

    registerApplicationLifecycle(
      ipc,
      createApplicationLifecycle(app, vi.fn(), ["/opt/OmegaT/omegat"]),
    );

    expect([...handlers.keys()]).toEqual(["app-quit", "app-relaunch"]);
  });

  it("preserves packaged arguments, stops the sidecar, and exits after scheduling relaunch", () => {
    const calls: string[] = [];
    const app: LifecycleApp = {
      relaunch: vi.fn((options) => {
        calls.push(`relaunch:${JSON.stringify(options.args)}`);
      }),
      quit: vi.fn(() => {
        calls.push("quit");
      }),
      exit: vi.fn((code) => {
        calls.push(`exit:${code}`);
      }),
    };
    const lifecycle = createApplicationLifecycle(
      app,
      () => calls.push("stop-sidecar"),
      [
        "/opt/OmegaT/omegat",
        "--config-dir=/home/user/.omegat",
        "--scripts-dir=/home/user/scripts",
        "/home/user/project",
      ],
    );

    lifecycle.relaunch();

    expect(calls).toEqual([
      'relaunch:["--config-dir=/home/user/.omegat","--scripts-dir=/home/user/scripts","/home/user/project"]',
      "stop-sidecar",
      "exit:0",
    ]);
    expect(app.quit).not.toHaveBeenCalled();
  });

  it("stops the sidecar before a normal quit", () => {
    const calls: string[] = [];
    const app: LifecycleApp = {
      relaunch: vi.fn(),
      quit: vi.fn(() => calls.push("quit")),
      exit: vi.fn(),
    };
    const lifecycle = createApplicationLifecycle(
      app,
      () => calls.push("stop-sidecar"),
      ["/opt/OmegaT/omegat"],
    );

    lifecycle.quit();

    expect(calls).toEqual(["stop-sidecar", "quit"]);
    expect(app.relaunch).not.toHaveBeenCalled();
    expect(app.exit).not.toHaveBeenCalled();
  });
});
