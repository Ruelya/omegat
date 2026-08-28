// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Loaded-entry window used only by the headless EditorController.
 *
 * The mounted renderer has its own immutable RendererPageProjection. Keeping
 * this state in a separate object prevents controller navigation/document
 * state from becoming a second renderer page model.
 */
export class HeadlessLoadedWindow {
  private visible: number[] = [];
  private first = -1;
  private last = -1;
  private radius = 25;
  private markerKeys = new Set<string>();
  private generation = 0;

  rebuild<T>(entries: readonly T[], allowed: (entry: T, index: number) => boolean): void {
    this.visible = entries.flatMap((entry, index) => allowed(entry, index) ? [index] : []);
    if (
      !this.visible.includes(this.first)
      || !this.visible.includes(this.last)
      || this.visible.indexOf(this.first) > this.visible.indexOf(this.last)
    ) {
      this.first = -1;
      this.last = -1;
    }
  }

  clear(): void {
    this.visible = [];
    this.first = -1;
    this.last = -1;
    this.invalidate();
  }

  invalidate(): void {
    this.markerKeys.clear();
    this.generation += 1;
  }

  currentGeneration(): number {
    return this.generation;
  }

  visibleIndices(): readonly number[] {
    return this.visible;
  }

  contains(index: number): boolean {
    return this.visible.includes(index);
  }

  firstVisible(): number | undefined {
    return this.visible[0];
  }

  findVisible(predicate: (index: number) => boolean): number | undefined {
    return this.visible.find(predicate);
  }

  visibleSet(): ReadonlySet<number> {
    return new Set(this.visible);
  }

  getRange(): { first: number; last: number } {
    return { first: this.first, last: this.last };
  }

  setRadius(radius: number, activeIndex: number): void {
    this.radius = Math.max(0, Math.floor(radius));
    if (activeIndex >= 0) this.around(activeIndex);
  }

  around(index: number, radius = this.radius): void {
    const visiblePosition = this.visible.indexOf(index);
    if (visiblePosition < 0) {
      this.first = -1;
      this.last = -1;
      return;
    }
    const first = Math.max(0, visiblePosition - radius);
    const last = Math.min(this.visible.length - 1, visiblePosition + radius);
    this.first = this.visible[first]!;
    this.last = this.visible[last]!;
  }

  loadedIndices(): number[] {
    if (this.first < 0 || this.last < this.first) return [];
    const first = this.visible.indexOf(this.first);
    const last = this.visible.indexOf(this.last);
    return first < 0 || last < first
      ? []
      : this.visible.slice(first, last + 1);
  }

  loadUp(count: number): number {
    const first = this.visible.indexOf(this.first);
    if (first <= 0) return 0;
    const next = Math.max(0, first - Math.max(0, Math.floor(count)));
    this.first = this.visible[next]!;
    return first - next;
  }

  loadDown(count: number): number {
    const last = this.visible.indexOf(this.last);
    if (last < 0 || last >= this.visible.length - 1) return 0;
    const next = Math.min(
      this.visible.length - 1,
      last + Math.max(0, Math.floor(count)),
    );
    this.last = this.visible[next]!;
    return next - last;
  }

  hasMoreBefore(): boolean {
    return this.visible.indexOf(this.first) > 0;
  }

  hasMoreAfter(): boolean {
    const last = this.visible.indexOf(this.last);
    return last >= 0 && last < this.visible.length - 1;
  }

  synchronizeMarkerKeys(keys: readonly string[]): boolean {
    if (
      keys.length === this.markerKeys.size
      && keys.every((key) => this.markerKeys.has(key))
    ) {
      return false;
    }
    this.markerKeys = new Set(keys);
    this.generation += 1;
    return true;
  }

  clearRange(): void {
    this.first = -1;
    this.last = -1;
  }
}
