import { describe, expect, test } from "vitest";

import {
  DISTINCT_GROUPS,
  THEME_PREFERENCES,
  THEME_STORAGE_KEY,
  parsePreference,
  resolveTheme,
  themeAttribute,
  themeScript,
} from "./theme";
import { deltaE2000, type Vision } from "./distinct";
import { COLOR_TOKENS, PALETTE, type Theme } from "./tokens";

describe("the theme preference", () => {
  test("a_stored_choice_reads_back_and_anything_else_is_system", () => {
    expect(parsePreference("system")).toBe("system");
    expect(parsePreference("light")).toBe("light");
    expect(parsePreference("dark")).toBe("dark");
    for (const raw of [null, "", "Dark", "sepia", "light "]) {
      expect(parsePreference(raw)).toBe("system");
    }
  });

  test("system_follows_the_scheme_and_a_pin_wins", () => {
    for (const preference of THEME_PREFERENCES) {
      for (const systemDark of [false, true]) {
        const expected =
          preference === "system"
            ? systemDark
              ? "dark"
              : "light"
            : preference;
        expect(resolveTheme(preference, systemDark)).toBe(expected);
      }
    }
  });

  test("the_attribute_is_the_pin_or_nothing", () => {
    expect(themeAttribute("system")).toBeNull();
    expect(themeAttribute("light")).toBe("light");
    expect(themeAttribute("dark")).toBe("dark");
  });
});

describe("the pre-paint script", () => {
  test("the_pre_paint_script_sets_a_stored_pin_and_survives_blocked_storage", () => {
    const src = themeScript();
    const asked: string[] = [];
    const run = (getItem: (key: string) => string | null) => {
      const doc = {
        documentElement: { dataset: {} as Record<string, string> },
      };
      // The script is a classic script's source; running it as one is the test.
      /* eslint-disable @typescript-eslint/no-implied-eval, @typescript-eslint/no-unsafe-call */
      new Function("localStorage", "document", src)(
        {
          getItem: (key: string) => {
            asked.push(key);
            return getItem(key);
          },
        },
        doc,
      );
      /* eslint-enable @typescript-eslint/no-implied-eval, @typescript-eslint/no-unsafe-call */
      return doc.documentElement.dataset.theme;
    };
    expect(run(() => "dark")).toBe("dark");
    expect(run(() => "light")).toBe("light");
    for (const stored of ["system", null, "sepia"]) {
      expect(run(() => stored)).toBeUndefined();
    }
    expect(
      run(() => {
        throw new Error("storage is blocked");
      }),
    ).toBeUndefined();
    expect(new Set(asked)).toEqual(new Set([THEME_STORAGE_KEY]));
    expect(src.length).toBeLessThan(400);
    expect(src.startsWith("(()=>{")).toBe(true);
    expect(src.endsWith("})()")).toBe(true);
  });
});

/** Every pair of a group that misses its bar in some theme and vision, as a line each. */
function failures(name: string): string[] {
  const group = DISTINCT_GROUPS.find((g) => g.name === name);
  if (group === undefined) {
    return [`no group named ${name}`];
  }
  const pairs: [
    (typeof group.tokens)[number],
    (typeof group.tokens)[number],
  ][] = [];
  group.tokens.forEach((x, i) => {
    group.tokens.slice(i + 1).forEach((y) => pairs.push([x, y]));
    group.against.forEach((y) => pairs.push([x, y]));
  });
  const out: string[] = [];
  for (const theme of Object.keys(PALETTE) as Theme[]) {
    for (const [vision, bar] of Object.entries(group.bars) as [
      Vision,
      number,
    ][]) {
      for (const [x, y] of pairs) {
        const d = deltaE2000(
          PALETTE[theme][x],
          PALETTE[theme][y],
          vision,
          PALETTE[theme].card,
        );
        if (d < bar) {
          out.push(`${theme} ${vision} ${x}/${y}: ΔE ${d.toFixed(1)} < ${bar}`);
        }
      }
    }
  }
  return out;
}

describe("the distinctness bars", () => {
  test("tones_stay_distinct_in_every_theme", () => {
    expect(DISTINCT_GROUPS.some((g) => g.name === "tones")).toBe(true);
    expect(failures("tones")).toEqual([]);
  });

  test("states_stay_distinct_in_every_theme", () => {
    expect(DISTINCT_GROUPS.some((g) => g.name === "states")).toBe(true);
    expect(failures("states")).toEqual([]);
  });

  test("lens_yarn_stays_distinct_in_every_theme", () => {
    expect(DISTINCT_GROUPS.some((g) => g.name === "yarn")).toBe(true);
    expect(failures("yarn")).toEqual([]);
  });

  test("every_distinct_group_holds_two_tokens_or_more", () => {
    for (const group of DISTINCT_GROUPS) {
      expect(group.tokens.length).toBeGreaterThanOrEqual(2);
      for (const token of [...group.tokens, ...group.against]) {
        expect(COLOR_TOKENS).toContain(token);
      }
    }
    expect(DISTINCT_GROUPS.map((g) => g.name)).toEqual([
      "tones",
      "states",
      "yarn",
    ]);
  });
});
