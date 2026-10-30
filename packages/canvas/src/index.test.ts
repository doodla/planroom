import { expect, test } from "vitest";

import { name } from "./index";

test("the package names itself", () => {
  expect(name).toBe("@planroom/canvas");
});

test("the_canvas_reads_its_lines_from_the_engines_canvas_table", async () => {
  const { CANVAS_STRINGS } = await import("@planroom/contract");
  expect(CANVAS_STRINGS.levels_sheet).toBe("Go to a level");
  expect(CANVAS_STRINGS.back_to).toContain("{place}");
  expect(CANVAS_STRINGS.open_block).toContain("{name}");
  for (const key of Object.keys(CANVAS_STRINGS)) {
    expect(key).not.toMatch(/_(label|next|action)$/);
  }
});
