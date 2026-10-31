/**
 * The workspace's pending store: hand edits the canvas has not yet seen the engine confirm. They
 * live in the origin's `localStorage` under the workspace's ID, so a reload, a closed tab or a
 * `serve` restart keeps them, and another workspace's tabs never send them (DR-8, DR-73).
 */

import type { Input } from "@planroom/contract";

import type { StorageLike } from "./tab";

export interface PendingEdit {
  /** The write's idempotency key: a flush that repeats it replays instead of writing twice. */
  readonly idem: string;
  /** The ID of the tab that made the edit, which a flush by any tab keeps. */
  readonly client: string;
  /** The request, `base` and `client` included, without `idem`. */
  readonly request: Input.EditReq;
  /** The engine-authored or canvas-authored line "Your edits" shows. */
  readonly line: string;
  /** When the edit was made, in milliseconds. */
  readonly at: number;
}

export const pendingKey = (workspace: string): string =>
  `planroom.pending.${workspace}`;

export interface PendingStore {
  list(): PendingEdit[];
  add(edit: PendingEdit): void;
  remove(idem: string): void;
}

// What a page holds for a workspace whose storage refuses reads or writes: it then serves every
// later call for that workspace, so the page keeps its edits for its lifetime.
const inMemory = new Map<string, PendingEdit[]>();

const order = (a: PendingEdit, b: PendingEdit): number =>
  a.at - b.at || (a.idem < b.idem ? -1 : a.idem > b.idem ? 1 : 0);

function parse(raw: string | null): PendingEdit[] {
  if (raw === null) {
    return [];
  }
  try {
    const value: unknown = JSON.parse(raw);
    return Array.isArray(value) ? (value as PendingEdit[]) : [];
  } catch {
    return [];
  }
}

/**
 * The pending store of one workspace. Each call re-reads the key and writes its change back
 * before it returns, so two tabs of the origin interleave one call at a time.
 */
export function pendingStore(
  storage: StorageLike | null,
  workspace: string,
): PendingStore {
  const key = pendingKey(workspace);

  /** Apply `change` to the stored list and return the result; a throw switches to memory. */
  const update = (change: (list: PendingEdit[]) => PendingEdit[]) => {
    const held = inMemory.get(workspace);
    if (held !== undefined || storage === null) {
      const next = change(held ?? []);
      inMemory.set(workspace, next);
      return next;
    }
    try {
      const next = change(parse(storage.getItem(key)));
      storage.setItem(key, JSON.stringify(next));
      return next;
    } catch {
      const next = change(inMemory.get(workspace) ?? []);
      inMemory.set(workspace, next);
      return next;
    }
  };

  return {
    list() {
      const held = inMemory.get(workspace);
      if (held !== undefined) {
        return [...held].sort(order);
      }
      try {
        return parse(storage?.getItem(key) ?? null).sort(order);
      } catch {
        return [];
      }
    },
    add(edit) {
      update((list) => [...list.filter((e) => e.idem !== edit.idem), edit]);
    },
    remove(idem) {
      update((list) => list.filter((e) => e.idem !== idem));
    },
  };
}
