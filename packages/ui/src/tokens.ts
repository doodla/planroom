/**
 * Design tokens (docs/contributing/design-language.md): the palette in both themes, type, shape
 * and motion, as TypeScript constants and as the CSS variables every web surface loads.
 *
 * The dark theme is designed, not inverted: each token has its own dark value. A surface styles a
 * block from the view model's `tone`, never from a kind ID (DR-46).
 */

import type { Tone, Yarn } from "@planroom/contract";

/** The two themes. */
export type Theme = "light" | "dark";

/** A color in `#rrggbb`, or `#rrggbb` with an opacity over whatever sits below it. */
export interface Color {
  readonly hex: string;
  readonly alpha: number;
}

/** The light palette, by CSS variable name without the leading `--`. */
const LIGHT = {
  table: { hex: "#cfe6dc", alpha: 1 },
  card: { hex: "#e3f1ea", alpha: 1 },
  ink: { hex: "#1b2a33", alpha: 1 },
  muted: { hex: "#4c6170", alpha: 1 },
  chalk: { hex: "#2d5a4a", alpha: 1 },
  "block-service": { hex: "#e8492b", alpha: 1 },
  "block-library": { hex: "#2f5bd3", alpha: 1 },
  "block-store": { hex: "#f2b632", alpha: 1 },
  "block-infra": { hex: "#6b8f71", alpha: 1 },
  "block-service-border": { hex: "#bc2f14", alpha: 1 },
  "block-library-border": { hex: "#1f43a8", alpha: 1 },
  "block-store-border": { hex: "#835c08", alpha: 1 },
  "block-infra-border": { hex: "#4f6953", alpha: 1 },
  "yarn-teal": { hex: "#0b6b70", alpha: 1 },
  "yarn-indigo": { hex: "#4b3fb8", alpha: 1 },
  "yarn-ochre": { hex: "#7a5200", alpha: 1 },
  "yarn-plum": { hex: "#79177b", alpha: 1 },
  "yarn-berry": { hex: "#b0175f", alpha: 1 },
  "yarn-copper": { hex: "#a3360f", alpha: 1 },
  pass: { hex: "#1a6b43", alpha: 1 },
  fail: { hex: "#b3261e", alpha: 1 },
  wait: { hex: "#6b5c0c", alpha: 1 },
} as const satisfies Record<string, Color>;

/** The dark palette: designed, not inverted. */
const DARK = {
  table: { hex: "#10231d", alpha: 1 },
  card: { hex: "#17312a", alpha: 1 },
  ink: { hex: "#e4f1ea", alpha: 1 },
  muted: { hex: "#9db8ab", alpha: 1 },
  chalk: { hex: "#e4f1ea", alpha: 0.7 },
  "block-service": { hex: "#ff7a5c", alpha: 1 },
  "block-library": { hex: "#7f9dff", alpha: 1 },
  "block-store": { hex: "#f5c95c", alpha: 1 },
  "block-infra": { hex: "#8fb596", alpha: 1 },
  "block-service-border": { hex: "#ffa48e", alpha: 1 },
  "block-library-border": { hex: "#aabdff", alpha: 1 },
  "block-store-border": { hex: "#f8dc96", alpha: 1 },
  "block-infra-border": { hex: "#b4cfb8", alpha: 1 },
  "yarn-teal": { hex: "#5fd6d6", alpha: 1 },
  "yarn-indigo": { hex: "#b0a8ff", alpha: 1 },
  "yarn-ochre": { hex: "#f0b860", alpha: 1 },
  "yarn-plum": { hex: "#e478fc", alpha: 1 },
  "yarn-berry": { hex: "#ff8fc0", alpha: 1 },
  "yarn-copper": { hex: "#ff9d6e", alpha: 1 },
  pass: { hex: "#5fd39a", alpha: 1 },
  fail: { hex: "#ff8a80", alpha: 1 },
  wait: { hex: "#e6cf6a", alpha: 1 },
} as const satisfies Record<keyof typeof LIGHT, Color>;

/** Every palette token, by its CSS variable name without the leading `--`. */
export const PALETTE = { light: LIGHT, dark: DARK } as const;

/** A palette token's name. */
export type ColorToken = keyof (typeof PALETTE)["light"];

/** The palette's names, in the order the design language lists them. */
export const COLOR_TOKENS = /* @__PURE__ */ Object.keys(
  LIGHT,
) as readonly ColorToken[];

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
  "yarn-teal",
  "yarn-indigo",
  "yarn-ochre",
  "yarn-plum",
  "yarn-berry",
  "yarn-copper",
  "pass",
  "fail",
  "wait",
] as const satisfies readonly ColorToken[];

/** The contrast bar (WCAG 2): text, then marks. Block borders hold the text bar [DR-77]. */
export const CONTRAST_BAR = { text: 4.5, mark: 3, border: 4.5 } as const;

/**
 * The palette tokens that paint a block of each tone a pack's kind may declare [DR-46]. Typed
 * against the contract's `Tone`, so a tone the contract adds fails to type-check until it has
 * tokens here.
 */
export const TONE_TOKENS = {
  service: { fill: "block-service", border: "block-service-border" },
  library: { fill: "block-library", border: "block-library-border" },
  store: { fill: "block-store", border: "block-store-border" },
  infra: { fill: "block-infra", border: "block-infra-border" },
} as const satisfies Record<Tone, { fill: ColorToken; border: ColorToken }>;

/** The CSS variables that paint a block of `tone`; no tone draws as a plain card. */
export function toneVars(tone: Tone | null): { fill: string; border: string } {
  if (tone === null) {
    return { fill: "var(--card)", border: "var(--muted)" };
  }
  const { fill, border } = TONE_TOKENS[tone];
  return { fill: `var(--${fill})`, border: `var(--${border})` };
}

/**
 * The palette token each yarn a lens may name draws its edges in. Typed against the contract's
 * `Yarn`, so a yarn the contract adds fails to type-check until it has a token here.
 */
export const YARN_TOKENS = {
  teal: "yarn-teal",
  indigo: "yarn-indigo",
  ochre: "yarn-ochre",
  plum: "yarn-plum",
  berry: "yarn-berry",
  copper: "yarn-copper",
} as const satisfies Record<Yarn, ColorToken>;

/** The CSS color a lens's edges draw in: its yarn's token, or the muted token with none (`--lens all`). */
export function yarnVar(yarn: Yarn | null): string {
  return yarn === null ? "var(--muted)" : `var(--${YARN_TOKENS[yarn]})`;
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
