import { expect, test } from "vitest";

import { fill, truncate, width } from "./text";

test("text that fits is left alone", () => {
  expect(truncate("storefront", 10)).toBe("storefront");
});

test("longer text is cut with an ellipsis inside the width", () => {
  expect(truncate("storefront-api", 10)).toBe("storefron…");
  expect(width(truncate("storefront-api", 10))).toBe(10);
  expect(truncate("abc", 0)).toBe("");
});

test("width counts code points, not UTF-16 units", () => {
  expect(width("▣ 🧱")).toBe(3);
  expect(truncate("🧱🧱🧱", 2)).toBe("🧱…");
});

test("fill_writes_each_value_once_left_to_right", () => {
  expect(fill("Back to the {level} level", { level: "system" })).toBe(
    "Back to the system level",
  );
  expect(
    fill("{first}–{last} of {total}", { first: 1, last: 7, total: 16 }),
  ).toBe("1–7 of 16");
  // A value that holds a placeholder is written as it is, never read again.
  expect(fill("{a} then {b}", { a: "{b}", b: "x" })).toBe("{b} then x");
  // An unknown name, a lone brace and an unclosed one stay as written.
  expect(fill("{nope} {a", { a: "1" })).toBe("{nope} {a");
  expect(fill("a } b {", {})).toBe("a } b {");
  // Only the template's own names: nothing from the object's prototype.
  expect(fill("{toString}", {})).toBe("{toString}");
});
