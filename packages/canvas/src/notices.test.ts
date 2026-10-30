import type { Warning } from "@planroom/contract";
import { expect, test } from "vitest";

import { noticesOf, shownNotices } from "./notices";

const warn = (message: string): Warning => ({
  code: "partial_scan",
  message,
  details: null,
});

test("notices_come_error_then_press_then_view_deduplicated_and_sanitized", () => {
  const notices = noticesOf(
    { line: "The store is locked.", hint: "planroom doctor" },
    [warn("Repo a is unborn."), warn("Shared line.")],
    [warn("Shared line."), warn("Scan was partial‮.")],
  );
  expect(notices.map((n) => n.line)).toEqual([
    "The store is locked.",
    "Repo a is unborn.",
    "Shared line.",
    expect.stringMatching(/^Scan was partial/),
  ]);
  expect(notices[0]?.hint).toBe("planroom doctor");
  expect(notices[1]?.hint).toBeNull();
  expect(notices[3]?.line).not.toContain("‮");
  // With no error the press comes first; with nothing, nothing.
  expect(noticesOf(null, [warn("p")], [warn("v")]).map((n) => n.line)).toEqual([
    "p",
    "v",
  ]);
  expect(noticesOf(null, [], [])).toEqual([]);
});

test("at_most_three_notices_show_then_how_many_more", () => {
  const four = ["a", "b", "c", "d", "e"].map((line) => ({ line, hint: null }));
  const shown = shownNotices(four);
  expect(shown.shown.map((n) => n.line)).toEqual(["a", "b", "c"]);
  expect(shown.more).toBe(2);
  expect(shownNotices(four.slice(0, 2))).toEqual({
    shown: four.slice(0, 2),
    more: 0,
  });
  expect(shownNotices(four, 1).more).toBe(4);
});
