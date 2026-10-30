/**
 * The local canvas's theme host [DR-122]: it reads the pin kept under `planroom.theme`, sets or
 * clears `data-theme` on the page root, writes a choice back, and follows the other tabs of the
 * origin. The Artifact page never starts one: claude.ai owns `data-theme` there.
 */

import {
  THEME_STORAGE_KEY,
  parsePreference,
  themeAttribute,
  type ThemePreference,
} from "@planroom/ui";
import { createContext, useContext, useSyncExternalStore } from "react";

export interface ThemeEnv {
  /** The origin's `localStorage`, or nothing where storage is blocked. */
  readonly storage: Pick<Storage, "getItem" | "setItem" | "removeItem"> | null;
  readonly root: { readonly dataset: DOMStringMap };
  /** Calls `listen` on each `storage` event another tab of the origin raises. */
  readonly onStorage: (
    listen: (key: string | null, value: string | null) => void,
  ) => void;
}

export interface ThemeHost {
  readonly preference: () => ThemePreference;
  readonly choose: (preference: ThemePreference) => void;
  readonly subscribe: (fn: () => void) => () => void;
}

/** The browser's own pieces. */
export function browserThemeEnv(): ThemeEnv {
  // The accessor throws where site data is blocked; a choice then holds for this page only.
  const storage = ((): ThemeEnv["storage"] => {
    try {
      return window.localStorage;
    } catch {
      return null;
    }
  })();
  return {
    storage,
    root: document.documentElement,
    onStorage: (listen) =>
      window.addEventListener("storage", (e) => listen(e.key, e.newValue)),
  };
}

/** Apply the stored pin now, and keep the root and the store in step with every later choice. */
export function startTheme(env: ThemeEnv): ThemeHost {
  const subscribers = new Set<() => void>();
  let current: ThemePreference;

  const read = (): string | null => {
    try {
      return env.storage?.getItem(THEME_STORAGE_KEY) ?? null;
    } catch {
      return null;
    }
  };
  const apply = (preference: ThemePreference): void => {
    current = preference;
    const pin = themeAttribute(preference);
    if (pin === null) {
      delete env.root.dataset.theme;
    } else {
      env.root.dataset.theme = pin;
    }
    for (const fn of subscribers) {
      fn();
    }
  };

  apply(parsePreference(read()));

  env.onStorage((key, value) => {
    if (key === THEME_STORAGE_KEY) {
      apply(parsePreference(value));
    } else if (key === null) {
      // Another tab cleared the whole storage.
      apply(parsePreference(null));
    }
  });

  return {
    preference: () => current,
    choose: (preference) => {
      try {
        if (themeAttribute(preference) === null) {
          env.storage?.removeItem(THEME_STORAGE_KEY);
        } else {
          env.storage?.setItem(THEME_STORAGE_KEY, preference);
        }
      } catch {
        // The choice holds for this page only.
      }
      apply(preference);
    },
    subscribe: (fn) => {
      subscribers.add(fn);
      return () => void subscribers.delete(fn);
    },
  };
}

export const ThemeContext = createContext<ThemeHost | null>(null);

const noSubscribe = (): (() => void) => () => {};
const noPreference = (): ThemePreference => parsePreference(null);

/** The pin and a way to change it, or nothing where no host runs (the Artifact page). */
export function useThemeChoice(): {
  preference: ThemePreference;
  choose: (preference: ThemePreference) => void;
} | null {
  const host = useContext(ThemeContext);
  const preference = useSyncExternalStore(
    host?.subscribe ?? noSubscribe,
    host?.preference ?? noPreference,
  );
  return host === null ? null : { preference, choose: host.choose };
}
