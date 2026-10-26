import { expect, test } from "vitest";

import { BAND_KEYS, ROW_KEYS, rowKeys } from "./keys";

test("row keys run top to bottom from one pool of 12", () => {
  expect(ROW_KEYS).toHaveLength(12);
  expect(rowKeys(3)).toEqual(["5", "6", "7"]);
  expect(rowKeys(13).at(-1)).toBeUndefined();
  expect(rowKeys(0)).toEqual([]);
});

test("row keys never take a band key", () => {
  const band: readonly string[] = Object.values(BAND_KEYS);
  expect(ROW_KEYS.filter((k) => band.includes(k))).toEqual([]);
});
