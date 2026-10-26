/** The store as React reads it, and the one media query the layout switches on. */

import { createContext, useContext, useSyncExternalStore } from "react";
import { useStore } from "zustand";

import type { CanvasState, CanvasStore } from "./store";

export const StoreContext = createContext<CanvasStore | null>(null);

export function useCanvas<T>(selector: (state: CanvasState) => T): T {
  const store = useContext(StoreContext);
  if (store === null) {
    throw new Error("useCanvas outside the store's provider");
  }
  return useStore(store, selector);
}

/** The phone layout: one row on top, a bottom sheet below (design-language.md, "Touch"). */
export const PHONE = "(max-width: 719px)";

export function useMedia(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const list = window.matchMedia(query);
      list.addEventListener("change", onChange);
      return () => list.removeEventListener("change", onChange);
    },
    () => window.matchMedia(query).matches,
  );
}
