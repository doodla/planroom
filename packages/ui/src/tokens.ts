/**
 * Design tokens (docs/contributing/design-language.md): the palette in both themes, type, shape
 * and motion, as TypeScript constants and as the CSS variables every web surface loads.
 *
 * The dark theme is designed, not inverted: each token has its own dark value. A surface styles a
 * block from the view model's `tone`, never from a kind ID (DR-46).
 */

import type { Tone } from "@planroom/contract";

/** The two themes. */
export type Theme = "light" | "dark";

/** A color in `#rrggbb`, or `#rrggbb` with an opacity over whatever sits below it. */
export interface Color {
  readonly hex: string;
  readonly alpha: number;
}

const solid = (hex: string): Color => ({ hex, alpha: 1 });

/** Every palette token, by its CSS variable name without the leading `--`. */
export const PALETTE = {
  light: {
    table: solid("#cfe6dc"),
    card: solid("#e3f1ea"),
    ink: solid("#1b2a33"),
    muted: solid("#4c6170"),
    chalk: solid("#2d5a4a"),
    "block-service": solid("#e8492b"),
    "block-library": solid("#2f5bd3"),
    "block-store": solid("#f2b632"),
    "block-infra": solid("#6b8f71"),
    "block-service-border": solid("#bc2f14"),
    "block-library-border": solid("#1f43a8"),
    "block-store-border": solid("#835c08"),
    "block-infra-border": solid("#4f6953"),
    pass: solid("#1a6b43"),
    fail: solid("#b3261e"),
    wait: solid("#6b5c0c"),
  },
  dark: {
    table: solid("#10231d"),
    card: solid("#17312a"),
    ink: solid("#e4f1ea"),
    muted: solid("#9db8ab"),
    chalk: { hex: "#e4f1ea", alpha: 0.7 },
    "block-service": solid("#ff7a5c"),
    "block-library": solid("#7f9dff"),
    "block-store": solid("#f5c95c"),
    "block-infra": solid("#8fb596"),
    "block-service-border": solid("#ffa48e"),
    "block-library-border": solid("#aabdff"),
    "block-store-border": solid("#f8dc96"),
    "block-infra-border": solid("#b4cfb8"),
    pass: solid("#5fd39a"),
    fail: solid("#ff8a80"),
    wait: solid("#e6cf6a"),
  },
} as const satisfies Record<Theme, Record<string, Color>>;

/** A palette token's name. */
export type ColorToken = keyof (typeof PALETTE)["light"];

/** The palette's names, in the order the design language lists them. */
export const COLOR_TOKENS = Object.keys(PALETTE.light) as readonly ColorToken[];

/** The backgrounds every text and mark token is measured against. */
export const SURFACES = [
  "table",
  "card",
] as const satisfies readonly ColorToken[];

/** Tokens drawn as text: 4.5:1 on both surfaces. */
export const TEXT_TOKENS = [
  "ink",
  "muted",
] as const satisfies readonly ColorToken[];

/** Tokens drawn as marks (outlines, borders, tape, edges): 3:1 on both surfaces. */
export const MARK_TOKENS = [
  "chalk",
  "block-service-border",
  "block-library-border",
  "block-store-border",
  "block-infra-border",
  "pass",
  "fail",
  "wait",
] as const satisfies readonly ColorToken[];

/** The contrast bar (WCAG 2): text, then marks. Block borders hold the text bar [DR-77]. */
export const CONTRAST_BAR = { text: 4.5, mark: 3, border: 4.5 } as const;

/** The CSS variables that paint a block of `tone`; no tone draws as a plain card. */
export function toneVars(tone: Tone | null): { fill: string; border: string } {
  if (tone === null) {
    return { fill: "var(--card)", border: "var(--muted)" };
  }
  return {
    fill: `var(--block-${tone})`,
    border: `var(--block-${tone}-border)`,
  };
}

/** The three faces; each is OFL-1.1 and self-hosted by the canvas. */
export const FONTS = {
  display: {
    family: "Bricolage Grotesque",
    weights: [600, 800],
    fallback: "sans-serif",
  },
  text: {
    family: "Atkinson Hyperlegible Next",
    weights: [400, 600],
    fallback: "sans-serif",
  },
  code: { family: "JetBrains Mono", weights: [400], fallback: "monospace" },
} as const;

/** Shape and depth: chunky blocks, rectangular buttons. */
export const SHAPE = {
  blockRadius: 12,
  buttonRadius: 8,
  border: 2,
  shadowOffset: 3,
  /** The smallest touch target, in CSS pixels. */
  target: 44,
} as const;

/** Motion: the three named moments stay under `momentMs`; hover and focus feedback is quicker. */
export const MOTION = { momentMs: 280, feedbackMs: 120 } as const;

function cssColor({ hex, alpha }: Color): string {
  if (alpha === 1) {
    return hex;
  }
  const [r, g, b] = channels(hex);
  return `rgb(${r} ${g} ${b} / ${alpha})`;
}

function declarations(theme: Theme): string {
  return COLOR_TOKENS.map(
    (name) => `  --${name}: ${cssColor(PALETTE[theme][name])};`,
  ).join("\n");
}

/**
 * The tokens as CSS: the light palette on `:root`, the dark one under
 * `prefers-color-scheme: dark` unless `data-theme="light"` pins light, and under
 * `data-theme="dark"`; then type, shape and motion.
 */
export function tokensCss(): string {
  const font = (f: (typeof FONTS)[keyof typeof FONTS]) =>
    `"${f.family}", ${f.fallback}`;
  return `:root {
  color-scheme: light;
${declarations("light")}
  --font-display: ${font(FONTS.display)};
  --font-text: ${font(FONTS.text)};
  --font-code: ${font(FONTS.code)};
  --radius-block: ${SHAPE.blockRadius}px;
  --radius-button: ${SHAPE.buttonRadius}px;
  --border: ${SHAPE.border}px;
  --shadow-offset: ${SHAPE.shadowOffset}px;
  --target: ${SHAPE.target}px;
  --motion-moment: ${MOTION.momentMs}ms;
  --motion-feedback: ${MOTION.feedbackMs}ms;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    color-scheme: dark;
${indent(declarations("dark"))}
  }
}
:root[data-theme="dark"] {
  color-scheme: dark;
${declarations("dark")}
}
`;
}

function indent(text: string): string {
  return text
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
}

function channels(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** `fg` composited over the opaque `bg`. */
export function over(fg: Color, bg: Color): Color {
  const f = channels(fg.hex);
  const b = channels(bg.hex);
  const mixed = f.map((c, i) =>
    Math.round(c * fg.alpha + (b[i] ?? 0) * (1 - fg.alpha)),
  );
  return {
    hex: `#${mixed.map((c) => c.toString(16).padStart(2, "0")).join("")}`,
    alpha: 1,
  };
}

function luminance(color: Color): number {
  const [r, g, b] = channels(color.hex).map((c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The WCAG 2 contrast ratio of `fg`, composited over `bg`, against `bg`. */
export function contrast(fg: Color, bg: Color): number {
  const a = luminance(over(fg, bg));
  const b = luminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
