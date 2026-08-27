import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  DockPopupController,
  type DockMenuItem,
  type DockNotificationTone,
  type DockPopupSnapshot,
} from "../lib/dock-controllers";

export type DockFrameProps = {
  title: string;
  children: ReactNode;
  menu?: readonly DockMenuItem[];
  notification?: DockNotificationTone | null;
};

export function DockFrame({
  title,
  children,
  menu = [],
  notification = null,
}: DockFrameProps) {
  const popup = useRef(new DockPopupController());
  const [snapshot, setSnapshot] = useState<DockPopupSnapshot>(
    () => popup.current.snapshot(),
  );
  popup.current.update(menu);

  useEffect(() => {
    if (!snapshot.open) return;
    const close = () => setSnapshot(popup.current.close());
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", keydown);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", keydown);
    };
  }, [snapshot.open]);

  const open = (x: number, y: number) => {
    if (menu.length === 0) return;
    setSnapshot(popup.current.open(x, y));
  };

  return (
    <section
      className="dock"
      data-notification={notification ?? undefined}
      onContextMenu={(event) => {
        if (menu.length === 0) return;
        event.preventDefault();
        open(event.clientX, event.clientY);
      }}
    >
      <div className="pane-h">
        <span>{title}</span>
        {notification && (
          <span className="dock-notification sr-only" role="status" aria-live="polite">
            {notification === "hit" ? `${title}: results available` : `${title}: no results`}
          </span>
        )}
        {menu.length > 0 && (
          <button
            type="button"
            className="dock-menu-trigger"
            aria-label={`${title} menu`}
            aria-haspopup="menu"
            aria-expanded={snapshot.open}
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              open(rect.right, rect.bottom);
            }}
          >
            ⋮
          </button>
        )}
      </div>
      <div className="dock-body">{children}</div>
      {snapshot.open && (
        <div
          className="dock-popup"
          role="menu"
          aria-label={`${title} menu`}
          style={{ left: snapshot.x, top: snapshot.y }}
          onPointerDown={(event) => event.stopPropagation()}
        >
          {snapshot.items.map((item) => (
            <button
              key={item.id}
              type="button"
              role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
              aria-checked={item.checked}
              disabled={item.disabled}
              className={item.separatorBefore ? "separator" : undefined}
              onClick={() => {
                popup.current.invoke(item.id);
                setSnapshot(popup.current.snapshot());
              }}
            >
              {item.checked !== undefined && <span aria-hidden>{item.checked ? "✓" : ""}</span>}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
