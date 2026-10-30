import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { CANVAS_STRINGS } from "@planroom/contract";
import { expect, test } from "vitest";

/** Each spec beside this file, by name, with its source. */
const specs = readdirSync(import.meta.dirname)
  .filter((f) => f.endsWith(".spec.ts"))
  .map((f) => [f, readFileSync(join(import.meta.dirname, f), "utf8")] as const);

test("every_spec_reads_the_canvas_lines_from_canvas_strings", () => {
  expect(specs.length).toBeGreaterThan(0);
  const spelled: string[] = [];
  for (const [file, src] of specs) {
    for (const [key, line] of Object.entries(CANVAS_STRINGS)) {
      // A template's whole text never appears in a spec: it reads filled.
      if (line.includes("{")) {
        continue;
      }
      if (['"', "'", "`"].some((q) => src.includes(`${q}${line}${q}`))) {
        spelled.push(`${file} spells out CANVAS_STRINGS.${key}`);
      }
    }
  }
  expect(spelled).toEqual([]);
});
