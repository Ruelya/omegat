/**
 * Java `org.omegat.util.gui.Styles.EditorColor`.
 * Painters must call `getColor()` at mark time so preference changes apply
 * without reconstructing the marker (`MarkerColorFreshnessTest`).
 */
export class EditorColor {
  static readonly COLOR_MARK_COMES_FROM_TM_XICE = new EditorColor("#c45c26");
  static readonly COLOR_MARK_COMES_FROM_TM_X100PC = new EditorColor("#2e7d32");
  static readonly COLOR_MARK_COMES_FROM_TM_XAUTO = new EditorColor("#1565c0");
  static readonly COLOR_MARK_COMES_FROM_TM_XENFORCED = new EditorColor("#6a1b9a");

  private override: string | null = null;

  constructor(readonly defaultColor: string) {}

  getColor(): string {
    return this.override ?? this.defaultColor;
  }

  setColor(color: string | null): void {
    this.override = color;
  }
}

export type LinkedTm = "xICE" | "x100PC" | "xAUTO" | "xENFORCED";

export function colorForLinked(linked: LinkedTm): EditorColor {
  switch (linked) {
    case "xICE":
      return EditorColor.COLOR_MARK_COMES_FROM_TM_XICE;
    case "x100PC":
      return EditorColor.COLOR_MARK_COMES_FROM_TM_X100PC;
    case "xAUTO":
      return EditorColor.COLOR_MARK_COMES_FROM_TM_XAUTO;
    default:
      return EditorColor.COLOR_MARK_COMES_FROM_TM_XENFORCED;
  }
}
