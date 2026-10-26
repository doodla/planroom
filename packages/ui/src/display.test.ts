import { expect, test } from "vitest";

import { sanitize } from "./display";

test("plain text passes through, emoji and accents included", () => {
  expect(sanitize("orders-db · café 🧱")).toBe("orders-db · café 🧱");
});

test("C0, DEL, C1 and ESC show as visible escapes", () => {
  expect(sanitize("a\x1b[31mred")).toBe("a\\x1b[31mred");
  expect(sanitize("tab\there\nnext")).toBe("tab\\x09here\\x0anext");
  expect(sanitize("del\x7f c1\x9b")).toBe("del\\x7f c1\\x9b");
});

test("bidi controls and zero-width characters show as markers", () => {
  expect(sanitize("evil\u202Etxt.exe")).toBe("evil⟨U+202E⟩txt.exe");
  expect(sanitize("zero\u200Bwidth\uFEFF")).toBe("zero⟨U+200B⟩width⟨U+FEFF⟩");
  expect(sanitize("\u2066isolate\u2069")).toBe("⟨U+2066⟩isolate⟨U+2069⟩");
});
