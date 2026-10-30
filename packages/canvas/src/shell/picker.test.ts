import { expect, test } from "vitest";

import { firstFocus, type FocusScope } from "./picker";

/** A scope whose `querySelector` answers the named selectors with a stand-in element each. */
function scope(found: Record<string, string>): FocusScope {
  return {
    querySelector: (selector: string) => found[selector] ?? null,
  };
}

const radios = scope({
  '[aria-checked="true"]': "the checked theme radio",
  button: "the first theme radio",
});

test("the_sheet_focuses_its_own_current_entry_before_the_extra", () => {
  const lenses = scope({
    "button[aria-current]": "the current lens",
    button: "the first lens",
  });
  expect(firstFocus(lenses, radios)).toBe("the current lens");
  // No entry marked current: the first entry, still ahead of the extra.
  expect(firstFocus(scope({ button: "the first lens" }), radios)).toBe(
    "the first lens",
  );
  // No entry that can be pressed (none at all, or one lens drawn flat): the extra's checked
  // control, else its first button.
  expect(firstFocus(null, radios)).toBe("the checked theme radio");
  expect(firstFocus(scope({}), radios)).toBe("the checked theme radio");
  expect(firstFocus(null, scope({ button: "the first theme radio" }))).toBe(
    "the first theme radio",
  );
  expect(firstFocus(null, null)).toBeNull();
});
