/** Java `org.omegat.gui.editor.SegmentBuilder`. */
export type BuiltSegment = {
  source: string;
  translation: string;
  active: boolean;
  number: number;
};

export function buildSegment(number: number, source: string, translation: string, active: boolean): BuiltSegment {
  return { number, source, translation, active };
}
