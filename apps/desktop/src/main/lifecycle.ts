export type RelaunchOptions = {
  args: string[];
};

export type LifecycleApp = {
  relaunch: (options: RelaunchOptions) => void;
  quit: () => void;
  exit: (code: number) => void;
};

export type LifecycleIpc = {
  handle: (channel: string, handler: () => void) => void;
};

export type ApplicationLifecycle = {
  quit: () => void;
  relaunch: () => void;
};

export function createApplicationLifecycle(
  app: LifecycleApp,
  stopSidecar: () => void,
  argv: readonly string[],
): ApplicationLifecycle {
  return {
    quit: () => {
      stopSidecar();
      app.quit();
    },
    relaunch: () => {
      app.relaunch({ args: argv.slice(1) });
      stopSidecar();
      app.exit(0);
    },
  };
}

export function registerApplicationLifecycle(
  ipc: LifecycleIpc,
  lifecycle: ApplicationLifecycle,
): void {
  ipc.handle("app-quit", lifecycle.quit);
  ipc.handle("app-relaunch", lifecycle.relaunch);
}
