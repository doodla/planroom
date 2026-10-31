/**
 * The tab's own writer ID. Each tab is its own writer instance, so two tabs of one person conflict
 * like two people (DR-5), and a flush of a tab's pending edits names the tab that made them, never
 * the flusher's (DR-73).
 */

export type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** Where the tab keeps its ID, in the tab's `sessionStorage`. */
export const TAB_KEY = "planroom.tab";

const TAB_ID = /^tab:[0-9a-f]{16}$/;

// Where storage is blocked, the page keeps one ID for its lifetime.
let pageTab: string | null = null;

type Random = (bytes: Uint8Array<ArrayBuffer>) => Uint8Array;

const mint = (random: Random): string =>
  `tab:${Array.from(random(new Uint8Array(8)), (b) =>
    b.toString(16).padStart(2, "0"),
  ).join("")}`;

/**
 * The tab's ID: the stored one when it has the form `tab:` and 16 hex digits, else a fresh one
 * from 8 random bytes, stored for the tab's next reload.
 */
export function tabId(
  storage: StorageLike | null,
  random: Random = (b) => crypto.getRandomValues(b),
): string {
  try {
    const stored = storage?.getItem(TAB_KEY) ?? null;
    if (stored !== null && TAB_ID.test(stored)) {
      return stored;
    }
    const minted = mint(random);
    storage?.setItem(TAB_KEY, minted);
    if (storage) {
      return minted;
    }
  } catch {
    // Storage blocked: fall through to the page's own ID.
  }
  pageTab ??= mint(random);
  return pageTab;
}
