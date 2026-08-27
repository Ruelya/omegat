import type { CSSProperties, ReactNode } from "react";
import { useApp } from "../store/app";
import type { DockLayout } from "../lib/layout";

export function Split({
  axis,
  ratio,
  field,
  children,
}: {
  axis: "h" | "v";
  ratio: number;
  field: keyof DockLayout;
  children: [ReactNode, ReactNode];
}) {
  const setLayout = useApp((s) => s.setLayout);
  const style: CSSProperties =
    axis === "h"
      ? { display: "grid", gridTemplateColumns: `${ratio}fr 6px ${1 - ratio}fr`, minHeight: 0, minWidth: 0, height: "100%" }
      : { display: "grid", gridTemplateRows: `${ratio}fr 6px ${1 - ratio}fr`, minHeight: 0, minWidth: 0, height: "100%" };
  return (
    <div className={`split split-${axis}`} style={style}>
      <div className="split-pane">{children[0]}</div>
      <div
        className={`splitter splitter-${axis}`}
        role="separator"
        onMouseDown={(ev) => {
          ev.preventDefault();
          const parent = (ev.target as HTMLElement).parentElement;
          if (!parent) return;
          const rect = parent.getBoundingClientRect();
          const move = (e: MouseEvent) => {
            const r =
              axis === "h" ? (e.clientX - rect.left) / rect.width : (e.clientY - rect.top) / rect.height;
            setLayout({ [field]: r } as Partial<DockLayout>);
          };
          const up = () => {
            window.removeEventListener("mousemove", move);
            window.removeEventListener("mouseup", up);
          };
          window.addEventListener("mousemove", move);
          window.addEventListener("mouseup", up);
        }}
      />
      <div className="split-pane">{children[1]}</div>
    </div>
  );
}
