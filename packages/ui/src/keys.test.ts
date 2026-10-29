import { expect, test } from "vitest";

import {
  ACTION_KEYS,
  BAND_KEYS,
  CARD_KEYS,
  KEY_MAP,
  ROW_KEYS,
  rowKeys,
} from "./keys";

test("row keys run top to bottom from one pool of 12", () => {
  expect(ROW_KEYS).toHaveLength(12);
  expect(rowKeys(3)).toEqual(["5", "6", "7"]);
  expect(rowKeys(13).at(-1)).toBeUndefined();
  expect(rowKeys(0)).toEqual([]);
});

test("the key map holds the card's four, the twelve row keys and the twenty action keys once each", () => {
  expect(ACTION_KEYS).toHaveLength(20);
  expect(KEY_MAP).toHaveLength(36);
  const keys = KEY_MAP.map((k) => k.key);
  expect(new Set(keys).size).toBe(36);
  expect(keys).toEqual([...CARD_KEYS, ...ROW_KEYS, ...ACTION_KEYS]);
  // The host takes one digit or one lowercase letter as a hotkey.
  for (const k of keys) expect(k).toMatch(/^[0-9a-z]$/);
  // The band's three are the card's first three.
  for (const k of Object.values(BAND_KEYS)) expect(CARD_KEYS).toContain(k);
  expect(KEY_MAP.find((k) => k.key === "e")?.section).toBe("list");
});

test("row keys never take a band key", () => {
  const band: readonly string[] = Object.values(BAND_KEYS);
  expect(ROW_KEYS.filter((k) => band.includes(k))).toEqual([]);
});
