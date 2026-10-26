import { expect, test } from "vitest";

import { truncate, width } from "./text";

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
