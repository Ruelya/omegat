/** Java `org.omegat.gui.editor.CollapsibleBar`. */

export const ARROW_EXPANDED = "▾";
export const ARROW_COLLAPSED = "▸";

export type CollapsibleBarState = {
  collapsed: boolean;
  title: string;
  summary: string;
  expanded: boolean;
  arrow: string;
  bodyVisible: boolean;
};

export abstract class CollapsibleBar {
  expanded = false;
  summary = "";
  body: string[] = [];

  constructor() {
    this.applyExpandedState(false);
  }

  protected abstract buildSummary(): string;

  getBody(): string[] {
    return this.body;
  }

  refreshSummary(): void {
    this.summary = this.buildSummary();
  }

  isExpanded(): boolean {
    return this.expanded;
  }

  setExpanded(expand: boolean): void {
    this.applyExpandedState(expand);
  }

  toggle(): void {
    this.applyExpandedState(!this.expanded);
  }

  getSummaryText(): string {
    return this.summary;
  }

  getArrow(): string {
    return this.expanded ? ARROW_EXPANDED : ARROW_COLLAPSED;
  }

  private applyExpandedState(expand: boolean): void {
    this.expanded = expand;
  }
}

export function toggleBar(bar: CollapsibleBarState): CollapsibleBarState {
  const expanded = bar.collapsed;
  return {
    ...bar,
    collapsed: !bar.collapsed,
    expanded,
    arrow: expanded ? ARROW_EXPANDED : ARROW_COLLAPSED,
    bodyVisible: expanded,
  };
}
