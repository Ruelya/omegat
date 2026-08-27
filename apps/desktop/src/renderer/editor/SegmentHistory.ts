/** Java `org.omegat.gui.editor.SegmentHistory`. */
export class SegmentHistory {
  back: number[] = [];
  forward: number[] = [];
  visit(n: number) {
    this.back.push(n);
    this.forward = [];
  }
  goBack(): number | undefined {
    const n = this.back.pop();
    if (n !== undefined) this.forward.push(n);
    return this.back.at(-1);
  }
  goForward(): number | undefined {
    const n = this.forward.pop();
    if (n !== undefined) this.back.push(n);
    return n;
  }
}
