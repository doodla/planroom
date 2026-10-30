/**
 * The theme preference and the checks every theme passes [DR-122]. A theme is a complete
 * palette (`PALETTE` in `tokens.ts`); the preference only says which one shows. The engine
 * never knows the preference: it lives in the viewer's browser.
 */
import type { Deficiency } from "./distinct";
import {
  TONE_TOKENS,
  YARN_TOKENS,
  type ColorToken,
  type Theme,
} from "./tokens";

/** What a person picks: follow the system's scheme, or pin one theme. */
export type ThemePreference = "system" | Theme;

export const THEME_PREFERENCES = [
  "system",
  "light",
  "dark",
] as const satisfies readonly ThemePreference[];

/** The browser storage key of the local canvas's pin; `system` removes it. */
export const THEME_STORAGE_KEY = "planroom.theme";

const PINS = ["light", "dark"] as const satisfies readonly Theme[];

/**
 * Reads a stored value back as a preference: the value when it is exactly one of
 * `THEME_PREFERENCES`, else `system`. The stored value carries no format tag because
 * anything unknown reads as `system`.
 */
export function parsePreference(raw: string | null): ThemePreference {
  return THEME_PREFERENCES.find((p) => p === raw) ?? "system";
}

/** The theme in force: the pin, or the system's scheme for `system`. */
export function resolveTheme(
  preference: ThemePreference,
  systemDark: boolean,
): Theme {
  if (preference === "system") {
    return systemDark ? "dark" : "light";
  }
  return preference;
}

/** The `data-theme` value for a preference: the pin, or `null` (no attribute) for `system`. */
export function themeAttribute(preference: ThemePreference): Theme | null {
  return preference === "system" ? null : preference;
}

/**
 * The source of the pre-paint script. The canvas build emits it as `assets/theme.js`, a
 * classic script that runs first in the head so a stored pin applies before the felt first
 * paints. It defines no global (one arrow IIFE), and a blocked storage throws inside the
 * `try` and sets nothing.
 */
export function themeScript(): string {
  const pinned = PINS.map((p) => `t===${JSON.stringify(p)}`).join("||");
  return `(()=>{try{const t=localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});if(${pinned})document.documentElement.dataset.theme=t}catch{}})()`;
}

/** A set of tokens told apart by colour, and the CIEDE2000 bar each vision holds them to [DR-122]. */
export interface DistinctGroup {
  readonly name: "tones" | "states" | "yarn";
  readonly tokens: readonly ColorToken[];
  /** Tokens each member is also held apart from, such as the card a block with no tone draws as. */
  readonly against: readonly ColorToken[];
  readonly bars: Readonly<
    { normal: number } & Partial<Record<Deficiency, number>>
  >;
}

// The calls carry the pure mark so a bundle that never runs the checks, such as the mod's,
// drops the list.
export const DISTINCT_GROUPS: readonly DistinctGroup[] = [
  {
    name: "tones",
    tokens: /* @__PURE__ */ Object.values(TONE_TOKENS).map((t) => t.fill),
    against: ["card"],
    bars: { normal: 20, protan: 10, deutan: 10, tritan: 10 },
  },
  // Tape carries +, − and ~, so states need no bar under a deficiency.
  {
    name: "states",
    tokens: ["pass", "fail", "wait"],
    against: [],
    bars: { normal: 20 },
  },
  // One lens shows at a time, named in the switcher.
  {
    name: "yarn",
    tokens: /* @__PURE__ */ Object.values(YARN_TOKENS),
    against: [],
    bars: { normal: 15 },
  },
];
