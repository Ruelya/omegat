/** Java `org.omegat.gui.editor.SegmentExportImport`. */

export type ExportedFiles = {
  "source.txt": string;
  "target.txt": string;
  "selection.txt": string;
};

const files: ExportedFiles = {
  "source.txt": "",
  "target.txt": "",
  "selection.txt": "",
};

export class SegmentExportImport {
  exportCurrentSegment(ste: { source: string; translation?: string | null }): void {
    files["source.txt"] = ste.source;
    files["target.txt"] = ste.translation ?? "target";
  }

  static flushExportedSegments(): void {
    files["source.txt"] = "";
    files["target.txt"] = "";
  }

  static exportCurrentSelection(selection: string): void {
    files["selection.txt"] = selection;
  }

  static read(name: keyof ExportedFiles): string {
    return files[name];
  }

  static exists(name: keyof ExportedFiles): boolean {
    return name in files;
  }
}

export function exportSegment(source: string, translation: string): string {
  const sei = new SegmentExportImport();
  sei.exportCurrentSegment({ source, translation });
  return `source\t${source}\ntarget\t${translation}\n`;
}

export function importSegment(raw: string): { source: string; translation: string } {
  const src = /source\t(.*)/.exec(raw)?.[1] ?? "";
  const tgt = /target\t(.*)/.exec(raw)?.[1] ?? "";
  return { source: src, translation: tgt };
}
