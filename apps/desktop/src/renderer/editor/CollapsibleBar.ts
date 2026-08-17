/** Java `org.omegat.gui.editor.CollapsibleBar`. */
export type CollapsibleBarState = { collapsed: boolean; title: string };

export function toggleBar(bar: CollapsibleBarState): CollapsibleBarState {
  return { ...bar, collapsed: !bar.collapsed };
}
