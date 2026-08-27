import { useEffect } from "react";
import { dispatchMenuAction } from "./actions";

export function useMenuBindings() {
  useEffect(() => {
    const off = window.omegat?.onMenu("menu:action", (action, payload) => {
      void dispatchMenuAction(String(action), payload);
    });
    return () => off?.();
  }, []);
}
