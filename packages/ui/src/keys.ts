/**
 * Hotkeys every surface shares (architecture.md, "Pane"; DR-54, DR-77).
 *
 * The band owns `1` to `3`: a bare digit at the empty prompt presses the band's Button. Row keys
 * select a row; action keys then act on the selected row. The host takes one digit or one
 * lowercase letter as a hotkey, 36 in all, and the key map holds every one of them once.
 */

/** The band's keys: the next action, the alternative, and "later". */
export const BAND_KEYS = {
  primary: "1",
  alternative: "2",
  later: "3",
} as const;

/**
 * The face-up card's keys: its choices take `1` to `4`, two to a row, and a card with one action
 * (the next action, the way back, a fallback's step) takes `1` (DR-54).
 */
export const CARD_KEYS = ["1", "2", "3", "4"] as const;

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

/** The pane's action keys, in the order of the pane's table (architecture.md, "Pane"). */
export const ACTION_KEYS = [
  "z",
  "x",
  "l",
  "p",
  "a",
  "o",
  "i",
  "h",
  "g",
  "d",
  "y",
  "n",
  "f",
  "w",
  "m",
  "s",
  "u",
  "e",
  "c",
  "t",
] as const;

export type ActionKey = (typeof ACTION_KEYS)[number];

/** The pane section a key belongs to. */
export type KeySection =
  | "card"
  | "focus"
  | "progress"
  | "diffs"
  | "drift"
  | "who"
  | "minimap"
  | "list"
  | "rows";

/** One key of the pane with what it does and the section it belongs to. */
export interface KeyBinding {
  key: string;
  action: string;
  section: KeySection;
}

const ACTIONS: Record<ActionKey, [string, KeySection]> = {
  z: ["up a level", "focus"],
  x: ["down into the selected row", "focus"],
  l: ["the lens picker", "focus"],
  p: ["the project picker", "focus"],
  a: ["accept the batch", "card"],
  o: ["open detail", "card"],
  i: ["the stage picker", "progress"],
  h: ["the hand-off picker", "progress"],
  g: ["merge the selected hand-off", "progress"],
  d: ["every pending diff in the list", "diffs"],
  y: ["accept the selected row", "diffs"],
  n: ["reject the selected row", "diffs"],
  f: ["every drift item in the list", "drift"],
  w: ["every presence and claim row in the list", "who"],
  m: ["toggle the mini-map", "minimap"],
  s: ["search", "list"],
  u: ["undo", "list"],
  e: ["open the canvas", "list"],
  c: ["curate", "list"],
  t: ["the remote picker", "list"],
};

/**
 * Every key of the pane: the card's four (the band's `1` next action, `2` alternative and `3`
 * later at the empty prompt), the twelve row keys, then the twenty action keys.
 */
export const KEY_MAP: readonly KeyBinding[] = [
  ...CARD_KEYS.map((key, i) => ({
    key,
    action:
      key === BAND_KEYS.primary
        ? "choose 1; the band's next action"
        : key === BAND_KEYS.alternative
          ? "choose 2; the band's alternative"
          : key === BAND_KEYS.later
            ? "choose 3; the band's later"
            : `choose ${String(i + 1)}`,
    section: "card" as const,
  })),
  ...ROW_KEYS.map((key) => ({
    key,
    action: "select its row",
    section: "rows" as const,
  })),
  ...ACTION_KEYS.map((key) => ({
    key,
    action: ACTIONS[key][0],
    section: ACTIONS[key][1],
  })),
];
