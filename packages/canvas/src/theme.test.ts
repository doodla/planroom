import { THEME_PREFERENCES, THEME_STORAGE_KEY } from "@planroom/ui";
import { describe, expect, test } from "vitest";

import { startTheme, type ThemeEnv } from "./theme";

function memoryStorage(initial: Record<string, string> = {}) {
  const items = new Map(Object.entries(initial));
  const writes: string[] = [];
  return {
    items,
    writes,
    getItem: (k: string) => items.get(k) ?? null,
    setItem: (k: string, v: string) => {
      writes.push(k);
      items.set(k, v);
    },
    removeItem: (k: string) => void items.delete(k),
  };
}

function setup(stored: Record<string, string> = {}) {
  const storage = memoryStorage(stored);
  const root = { dataset: {} as DOMStringMap };
  let listen: (key: string | null, value: string | null) => void = () => {};
  const env: ThemeEnv = {
    storage,
    root,
    onStorage: (fn) => {
      listen = fn;
    },
  };
  const host = startTheme(env);
  let calls = 0;
  host.subscribe(() => (calls += 1));
  return {
    host,
    storage,
    root,
    heard: (key: string | null, value: string | null) => listen(key, value),
    calls: () => calls,
  };
}

describe("the theme host", () => {
  test("a_stored_pin_applies_at_start_and_system_leaves_the_root_bare", () => {
    const pinned = setup({ [THEME_STORAGE_KEY]: "dark" });
    expect(pinned.root.dataset.theme).toBe("dark");
    expect(pinned.host.preference()).toBe("dark");

    const unknown = setup({ [THEME_STORAGE_KEY]: "sepia" });
    expect(unknown.root.dataset.theme).toBeUndefined();
    expect(unknown.host.preference()).toBe(THEME_PREFERENCES[0]);

    const nothing = setup();
    expect(nothing.root.dataset.theme).toBeUndefined();
    expect(nothing.host.preference()).toBe(THEME_PREFERENCES[0]);
  });

  test("choosing_a_theme_stores_it_and_sets_the_root", () => {
    const t = setup();
    t.host.choose("light");
    expect(t.storage.items.get(THEME_STORAGE_KEY)).toBe("light");
    expect(t.root.dataset.theme).toBe("light");
    expect(t.host.preference()).toBe("light");
    expect(t.calls()).toBe(1);
  });

  test("choosing_system_clears_the_root_and_the_store", () => {
    const t = setup({ [THEME_STORAGE_KEY]: "dark" });
    t.host.choose(THEME_PREFERENCES[0]);
    expect(t.storage.items.has(THEME_STORAGE_KEY)).toBe(false);
    expect(t.root.dataset.theme).toBeUndefined();
  });

  test("a_choice_in_another_tab_applies_here", () => {
    const t = setup();
    t.heard(THEME_STORAGE_KEY, "dark");
    expect(t.root.dataset.theme).toBe("dark");
    expect(t.calls()).toBe(1);
    expect(t.storage.writes).toEqual([]);

    t.heard("other.key", "light");
    expect(t.root.dataset.theme).toBe("dark");
    expect(t.calls()).toBe(1);

    t.heard(null, null);
    expect(t.root.dataset.theme).toBeUndefined();
    expect(t.host.preference()).toBe(THEME_PREFERENCES[0]);
  });

  test("with_storage_blocked_a_choice_holds_for_this_page", () => {
    const root = { dataset: {} as DOMStringMap };
    const blocked = startTheme({
      storage: null,
      root,
      onStorage: () => {},
    });
    blocked.choose("dark");
    expect(root.dataset.theme).toBe("dark");
    expect(blocked.preference()).toBe("dark");

    const boom = () => {
      throw new Error("blocked");
    };
    const throwing = { dataset: {} as DOMStringMap };
    let host: ReturnType<typeof startTheme> | undefined;
    expect(() => {
      host = startTheme({
        storage: { getItem: boom, setItem: boom, removeItem: boom },
        root: throwing,
        onStorage: () => {},
      });
      host.choose("light");
    }).not.toThrow();
    expect(throwing.dataset.theme).toBe("light");
    expect(host?.preference()).toBe("light");
  });
});
