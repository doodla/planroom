/**
 * Which rows of the pane take a row key, and how a list pages (architecture.md, "Pane"; DR-54,
 * DR-77). Every visible section shows its top items with direct row keys, taken top to bottom
 * from the one pool in `keys.ts`.
 */

import { ROW_KEYS, type RowKey } from "./keys";

/** A list shows this many rows a page, each with its row key. */
export const PAGE_ROWS = 7;

/**
 * The face-up card's keys: its choices take `1` to `4`, two to a row, and a card with one action
 * (the next action, the way back, a fallback's step) takes `1` (DR-54).
 */
export const CARD_KEYS = ["1", "2", "3", "4"] as const;

/** Diff lines that take a row key at most: the pane's top 3. */
const DIFF_ROWS = 3;

/** How many rows of each section an action can target, top to bottom. */
export interface KeyedSections {
  /** Pending diff lines; the top 3 take keys. */
  diffs: number;
  /** Whether the drift line has a top item. */
  drift: boolean;
  /** Whether the who line has a top row. */
  who: boolean;
  /** Rows in the list's page; the first 7 take keys. */
  list: number;
}

/** The row keys each section takes. */
export interface SectionKeys {
  diffs: RowKey[];
  drift: RowKey | undefined;
  who: RowKey | undefined;
  list: RowKey[];
}

/**
 * The pool's keys in order: up to 3 diff lines, then the drift line's top item when there is one,
 * the who line's top row when there is one, then up to 7 list rows. A section with nothing to
 * target takes no key, so the next one moves up.
 */
export function keyRows(sections: KeyedSections): SectionKeys {
  let next = 0;
  const take = (count: number): RowKey[] => {
    const keys = ROW_KEYS.slice(next, next + Math.max(0, count));
    next += keys.length;
    return keys;
  };
  const diffs = take(Math.min(sections.diffs, DIFF_ROWS));
  const drift = sections.drift ? take(1)[0] : undefined;
  const who = sections.who ? take(1)[0] : undefined;
  const list = take(Math.min(sections.list, PAGE_ROWS));
  return { diffs, drift, who, list };
}

/** One page of a list. `first` and `last` count from 1, and are 0 for an empty list. */
export interface Page<T> {
  rows: T[];
  /** The page shown, from 0, clamped to the pages there are. */
  index: number;
  /** At least 1: an empty list is one page with nothing on it. */
  pages: number;
  first: number;
  last: number;
  total: number;
}

/** Page `index` of `items`, `size` rows a page; an index past either end shows the nearest page. */
export function page<T>(
  items: readonly T[],
  index: number,
  size: number = PAGE_ROWS,
): Page<T> {
  const rows = Math.max(1, Math.floor(size));
  const total = items.length;
  const pages = Math.max(1, Math.ceil(total / rows));
  const at = Math.min(
    pages - 1,
    Math.max(0, Number.isFinite(index) ? Math.floor(index) : 0),
  );
  const start = at * rows;
  const shown = items.slice(start, start + rows);
  return {
    rows: shown,
    index: at,
    pages,
    first: shown.length === 0 ? 0 : start + 1,
    last: start + shown.length,
    total,
  };
}
