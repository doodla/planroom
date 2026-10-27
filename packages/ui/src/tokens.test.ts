import type { Tone } from "@planroom/contract";
import { describe, expect, test } from "vitest";

import {
  COLOR_TOKENS,
  CONTRAST_BAR,
  MARK_TOKENS,
  PALETTE,
  SURFACES,
  TEXT_TOKENS,
  TONE_TOKENS,
  contrast,
  over,
  tokensCss,
  toneVars,
  type Theme,
} from "./tokens";

const THEMES: Theme[] = ["light", "dark"];

describe("contrast_meets_the_bar", () => {
  for (const theme of THEMES) {
    for (const surface of SURFACES) {
      const bg = PALETTE[theme][surface];
      test.each(TEXT_TOKENS)(`${theme}: text --%s on --${surface}`, (token) => {
        expect(contrast(PALETTE[theme][token], bg)).toBeGreaterThanOrEqual(
          CONTRAST_BAR.text,
        );
      });
      test.each(MARK_TOKENS)(`${theme}: mark --%s on --${surface}`, (token) => {
        const bar = token.endsWith("-border")
          ? CONTRAST_BAR.border
          : CONTRAST_BAR.mark;
        expect(contrast(PALETTE[theme][token], bg)).toBeGreaterThanOrEqual(bar);
      });
    }
  }
});

test("contrast follows WCAG 2 and composites a translucent color first", () => {
  const black = { hex: "#000000", alpha: 1 };
  const white = { hex: "#ffffff", alpha: 1 };
  expect(contrast(black, white)).toBeCloseTo(21, 5);
  expect(contrast(white, white)).toBeCloseTo(1, 5);
  expect(over({ hex: "#ffffff", alpha: 0.5 }, black).hex).toBe("#808080");
});

test("white chalk on the light felt misses the bar, which is why light mode draws dark chalk", () => {
  expect(
    contrast({ hex: "#ffffff", alpha: 1 }, PALETTE.light.table),
  ).toBeLessThan(CONTRAST_BAR.mark);
});

test("both themes define every token", () => {
  expect(Object.keys(PALETTE.dark)).toEqual(COLOR_TOKENS);
});

test("the CSS holds every token in both themes, and a pinned theme wins over the system's", () => {
  const css = tokensCss();
  for (const token of COLOR_TOKENS) {
    expect(css.match(new RegExp(`--${token}: `, "g"))).toHaveLength(3);
  }
  expect(css).toContain("--table: #cfe6dc;");
  expect(css).toContain("--table: #10231d;");
  expect(css).toContain("--chalk: rgb(228 241 234 / 0.7);");
  expect(css).toContain(':root:not([data-theme="light"])');
  expect(css).toContain(':root[data-theme="dark"]');
  expect(css).toContain('--font-display: "Bricolage Grotesque", sans-serif;');
});

test("no cream or off-white background, and no banned face", () => {
  for (const theme of THEMES) {
    const [r, g, b] = [1, 3, 5].map((i) =>
      Number.parseInt(PALETTE[theme].table.hex.slice(i, i + 2), 16),
    );
    // The felt is green: its green channel leads, never a warm near-white.
    expect(g).toBeGreaterThan(r ?? 0);
    expect(g).toBeGreaterThan(b ?? 0);
  }
  expect(tokensCss()).not.toMatch(/Inter|Roboto|Arial|system-ui|Space Grotesk/);
});

describe("every_tone_has_a_token", () => {
  // Every tone the contract gives a kind, spelled out here so the type checker refuses a list
  // that misses one or names one the contract doesn't have.
  const tones = {
    service: true,
    library: true,
    store: true,
    infra: true,
  } as const satisfies Record<Tone, true>;

  test("the tone table names exactly the contract's tones", () => {
    expect(Object.keys(TONE_TOKENS).sort()).toEqual(Object.keys(tones).sort());
  });

  for (const theme of THEMES) {
    test.each(Object.keys(tones) as Tone[])(
      `${theme}: %s has a fill and a border token`,
      (tone) => {
        const { fill, border } = TONE_TOKENS[tone];
        expect(PALETTE[theme][fill].hex).toMatch(/^#[0-9a-f]{6}$/);
        expect(PALETTE[theme][border].hex).toMatch(/^#[0-9a-f]{6}$/);
        expect(MARK_TOKENS).toContain(border);
        expect(toneVars(tone)).toEqual({
          fill: `var(--${fill})`,
          border: `var(--${border})`,
        });
      },
    );
  }
});

test("a block takes its tone's fill and border, and no tone draws as a card", () => {
  expect(toneVars("store")).toEqual({
    fill: "var(--block-store)",
    border: "var(--block-store-border)",
  });
  expect(toneVars(null)).toEqual({
    fill: "var(--card)",
    border: "var(--muted)",
  });
});
