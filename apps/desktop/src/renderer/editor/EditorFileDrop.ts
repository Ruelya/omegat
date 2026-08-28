// SPDX-License-Identifier: GPL-3.0-or-later

export type EditorFileDrop =
  | { kind: "project"; root: string }
  | { kind: "files"; paths: string[] };

export type EditorFileDropHandlers = {
  openProject: (root: string) => void | Promise<void>;
  importFiles: (paths: string[]) => void | Promise<void>;
};

export type EditorFileDropResult =
  | { accepted: true; action: "open-project" | "import-files"; paths: string[] }
  | { accepted: false; action: "none"; paths: string[] };

export async function handleEditorFileDrop(
  drop: EditorFileDrop,
  projectLoaded: boolean,
  handlers: EditorFileDropHandlers,
): Promise<EditorFileDropResult> {
  if (drop.kind === "project" && drop.root.trim()) {
    await handlers.openProject(drop.root);
    return {
      accepted: true,
      action: "open-project",
      paths: [drop.root],
    };
  }
  const paths = drop.kind === "files"
    ? drop.paths.filter((path) => path.trim().length > 0)
    : [];
  if (!projectLoaded || paths.length === 0) {
    return { accepted: false, action: "none", paths };
  }
  await handlers.importFiles(paths);
  return { accepted: true, action: "import-files", paths };
}
