export type LifecycleApp = {
  relaunch: () => void;
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
): ApplicationLifecycle {
  return {
    quit: () => {
      stopSidecar();
      app.quit();
    },
    relaunch: () => {
      app.relaunch();
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
