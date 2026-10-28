import { expect, test } from "vitest";

import { BAND_KEYS, ROW_KEYS } from "./keys";
import { CARD_KEYS, keyRows, page, PAGE_ROWS } from "./rows";

test("row_keys_take_diffs_then_drift_then_who_then_seven_list_rows", () => {
  // Every section shows: three diffs, the drift and who top rows, then seven list rows.
  const all = keyRows({ diffs: 5, drift: true, who: true, list: 20 });
  expect(all.diffs).toEqual(["5", "6", "7"]);
  expect(all.drift).toBe("8");
  expect(all.who).toBe("9");
  expect(all.list).toEqual(["0", "b", "j", "k", "q", "r", "v"]);
  expect([...all.diffs, all.drift, all.who, ...all.list]).toEqual([
    ...ROW_KEYS,
  ]);
  // A section with nothing to target takes no key, and the next one moves up.
  const two = keyRows({ diffs: 2, drift: true, who: true, list: 0 });
  expect(two).toEqual({ diffs: ["5", "6"], drift: "7", who: "8", list: [] });
  const none = keyRows({ diffs: 0, drift: false, who: true, list: 2 });
  expect(none).toEqual({
    diffs: [],
    drift: undefined,
    who: "5",
    list: ["6", "7"],
  });
  expect(keyRows({ diffs: -1, drift: false, who: false, list: -3 })).toEqual({
    diffs: [],
    drift: undefined,
    who: undefined,
    list: [],
  });
});

test("the card's keys are 1 to 4 and never a row key", () => {
  expect(CARD_KEYS).toEqual(["1", "2", "3", "4"]);
  expect(CARD_KEYS[0]).toBe(BAND_KEYS.primary);
  const rows: readonly string[] = ROW_KEYS;
  expect(CARD_KEYS.filter((k) => rows.includes(k))).toEqual([]);
});

test("a_list_pages_at_seven_rows", () => {
  expect(PAGE_ROWS).toBe(7);
  const items = Array.from({ length: 16 }, (_, i) => i);
  expect(page(items, 0)).toEqual({
    rows: [0, 1, 2, 3, 4, 5, 6],
    index: 0,
    pages: 3,
    first: 1,
    last: 7,
    total: 16,
  });
  expect(page(items, 2)).toEqual({
    rows: [14, 15],
    index: 2,
    pages: 3,
    first: 15,
    last: 16,
    total: 16,
  });
  // The index is clamped to the pages there are.
  expect(page(items, 9).index).toBe(2);
  expect(page(items, -4).index).toBe(0);
  expect(page(items, 1.5).index).toBe(1);
  // A smaller size, and an empty list, which is one page with nothing on it.
  expect(page(items, 1, 5)).toMatchObject({ rows: [5, 6, 7, 8, 9], pages: 4 });
  expect(page([], 3)).toEqual({
    rows: [],
    index: 0,
    pages: 1,
    first: 0,
    last: 0,
    total: 0,
  });
});
