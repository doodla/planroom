import { describe, expect, test } from "vitest";

import {
  deltaE2000,
  deltaE2000Lab,
  simulateVision,
  type Deficiency,
  type Lab,
} from "./distinct";

const lab = (L: number, a: number, b: number): Lab => ({ L, a, b });

describe("distinctness math", () => {
  test("delta_e_2000_matches_the_published_pairs", () => {
    // Sharma, Wu and Dalal (2005), Table 1.
    const pairs: [Lab, Lab, number][] = [
      [lab(50, 2.6772, -79.7751), lab(50, 0, -82.7485), 2.0425],
      [lab(50, 0, 0), lab(50, -1, 2), 2.3669],
      [lab(50, 2.5, 0), lab(73, 25, -18), 27.1492],
      [lab(22.7233, 20.0904, -46.694), lab(23.0331, 14.973, -42.5619), 2.0373],
    ];
    for (const [x, y, expected] of pairs) {
      expect(deltaE2000Lab(x, y)).toBeCloseTo(expected, 4);
    }
    const [x, y] = pairs[0] as [Lab, Lab, number];
    expect(Math.abs(deltaE2000Lab(y, x) - deltaE2000Lab(x, y))).toBeLessThan(
      1e-9,
    );
  });

  test("simulated_vision_keeps_greys_and_pulls_red_and_green_together", () => {
    const kinds: Deficiency[] = ["protan", "deutan", "tritan"];
    for (const hex of ["#000000", "#808080", "#ffffff"]) {
      for (const kind of kinds) {
        const seen = simulateVision({ hex, alpha: 1 }, kind).hex;
        for (const at of [1, 3, 5]) {
          const before = Number.parseInt(hex.slice(at, at + 2), 16);
          const after = Number.parseInt(seen.slice(at, at + 2), 16);
          expect(Math.abs(after - before)).toBeLessThanOrEqual(1);
        }
      }
    }
    const red = { hex: "#e8492b", alpha: 1 };
    const green = { hex: "#4caf50", alpha: 1 };
    expect(deltaE2000(red, green, "deutan")).toBeLessThan(
      deltaE2000(red, green) / 2,
    );
  });
});
