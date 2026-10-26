/**
 * Hotkeys every surface shares (architecture.md, "Pane"; DR-54).
 *
 * The band owns `1` to `3`: a bare digit at the empty prompt presses the band's Button. Row keys
 * select a row; action keys then act on the selected row.
 */

/** The band's keys: the next action, the alternative, and "later". */
export const BAND_KEYS = {
  primary: "1",
  alternative: "2",
  later: "3",
} as const;

/** The pool of row keys, assigned top to bottom. */
export const ROW_KEYS = [
  "5",
  "6",
  "7",
  "8",
  "9",
  "0",
  "b",
  "j",
  "k",
  "q",
  "r",
  "v",
] as const;

export type RowKey = (typeof ROW_KEYS)[number];

/** The row keys for `count` rows, top to bottom; rows past the pool get none. */
export function rowKeys(count: number): (RowKey | undefined)[] {
  return Array.from({ length: Math.max(0, count) }, (_, i) => ROW_KEYS[i]);
}
