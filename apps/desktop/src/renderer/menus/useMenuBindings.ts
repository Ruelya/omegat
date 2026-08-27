import { useEffect } from "react";
import { useApp } from "../store/app";
import { dispatchMenuAction } from "./actions";

export function useMenuBindings() {
  const app = useApp();
  useEffect(() => {
    const off = window.omegat?.onMenu("menu:action", (action, payload) => {
      void dispatchMenuAction(String(action), payload);
    });
    return () => off?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app.screen, app.index, app.draft, app.marks, app.layout]);
}
