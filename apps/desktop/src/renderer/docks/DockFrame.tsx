import type { ReactNode } from "react";

export function DockFrame({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="dock">
      <div className="pane-h">{title}</div>
      <div className="dock-body">{children}</div>
    </section>
  );
}
