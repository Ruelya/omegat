/** Java `org.omegat.gui.editor.SegmentExportImport`. */
export function exportSegment(source: string, translation: string): string {
  return `source\t${source}\ntarget\t${translation}\n`;
}

export function importSegment(raw: string): { source: string; translation: string } {
  const src = /source\t(.*)/.exec(raw)?.[1] ?? "";
  const tgt = /target\t(.*)/.exec(raw)?.[1] ?? "";
  return { source: src, translation: tgt };
}
