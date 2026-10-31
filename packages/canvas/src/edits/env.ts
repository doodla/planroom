/**
 * What the canvas's hand edits read from the page: the origin's `localStorage` for the workspace's
 * pending store, the tab's `sessionStorage` for its ID, a UUID for each edit's idempotency key and
 * the clock. The store takes one so a test supplies its own (DR-8, DR-73).
 */

import type { StorageLike } from "./tab";

export interface EditsEnv {
  /** The origin's storage, shared by its tabs; null where blocked or absent. */
  local: StorageLike | null;
  /** The tab's own storage; null where blocked or absent. */
  session: StorageLike | null;
  /** A fresh idempotency key. */
  uuid: () => string;
  /** The time in milliseconds. */
  now: () => number;
}

// Reading a storage accessor throws where site data is blocked.
function accessor(read: () => Storage): StorageLike | null {
  try {
    return typeof window === "undefined" ? null : read();
  } catch {
    return null;
  }
}

export function browserEditsEnv(): EditsEnv {
  return {
    local: accessor(() => window.localStorage),
    session: accessor(() => window.sessionStorage),
    uuid: () => crypto.randomUUID(),
    now: () => Date.now(),
  };
}
