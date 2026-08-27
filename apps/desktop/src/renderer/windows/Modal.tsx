import type { ReactNode } from "react";
import type { WindowId } from "../lib/types";
import { useApp } from "../store/app";

export function Modal({
  id,
  title,
  children,
  wide,
}: {
  id: WindowId;
  title: string;
  children: ReactNode;
  wide?: boolean;
}) {
  const close = () => useApp.getState().openWindow(id, false);
  return (
    <div className="modal-bg" onClick={close}>
      <div className={`modal ${wide ? "wide" : ""}`} onClick={(e) => e.stopPropagation()}>
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}
