/**
 * Themes on the local canvas and the hosted Artifact page [DR-122]. On the local canvas the lens
 * switcher's sheet offers System, Light and Dark and the pin is applied before the first paint. On
 * the Artifact page claude.ai owns `data-theme`: the page follows it, loads no theme script and
 * offers no choice of its own.
 */

import type { Page } from "@playwright/test";
import { CANVAS_STRINGS } from "@planroom/contract";

import { installArtifactDb } from "../shims/artifact-db";
import { installClaude } from "../shims/claude";
import { hostArtifact, inline, shareFixture } from "./artifact-host";
import { DESKTOP, PHONE, expect, hitSize, openMap, test } from "./fixtures";

test.use({ viewport: DESKTOP });

test("the_artifact_follows_the_viewers_theme_and_offers_no_choice", async ({
  browser,
}) => {
  const ws = shareFixture();
  try {
    const cases = [
      [undefined, "dark", "dark"],
      ["light", "dark", "light"],
      ["dark", "light", "dark"],
    ] as const;
    for (const [theme, colorScheme, expected] of cases) {
      const host = await hostArtifact(ws.share, { theme });
      const context = await browser.newContext({ colorScheme });
      try {
        const page = await context.newPage();
        const errors: string[] = [];
        page.on("console", (m) => {
          if (m.type() === "error") {
            errors.push(m.text());
          }
        });
        page.on("pageerror", (e) => errors.push(e.message));
        const seeded: Record<string, Record<string, unknown>> = {};
        for (const w of inline(ws.share.db[0]!)) {
          seeded[`${w.collection}/${w.doc_id}`] = w.data!;
        }
        await page.addInitScript(installArtifactDb, {
          uid: "u_test",
          level: "interact" as const,
          docs: seeded,
        });
        await page.addInitScript(installClaude, {
          db: false,
          uid: "u_test",
          owner: false,
        });
        await page.goto(host.url);
        await page.locator(".map[data-ready]").waitFor();

        const scheme = await page.evaluate(
          () => getComputedStyle(document.documentElement).colorScheme,
        );
        expect(scheme, `${theme ?? "no"} theme under ${colorScheme}`).toBe(
          expected,
        );
        expect(host.requested.filter((p) => p.endsWith("theme.js"))).toEqual(
          [],
        );
        await expect(page.locator("[role=radiogroup]")).toHaveCount(0);
        const stored = await page.evaluate(() => {
          try {
            return localStorage.getItem("planroom.theme");
          } catch {
            return null;
          }
        });
        expect(stored).toBeNull();
        expect(errors).toEqual([]);
      } finally {
        await context.close();
        await host.close();
      }
    }
  } finally {
    ws.close();
  }
});

/** What the page's root says about its theme, and the page background that results. */
const root = (p: Page) =>
  p.evaluate(() => ({
    attr: document.documentElement.getAttribute("data-theme"),
    scheme: getComputedStyle(document.documentElement).colorScheme,
    bg: getComputedStyle(document.body).backgroundColor,
  }));

const switcher = (p: Page) =>
  p.locator(".top-row [data-concept=lenses]").getByRole("button", {
    name: CANVAS_STRINGS.lenses,
  });

test("the_canvas_follows_the_system_theme_by_default", async ({
  page,
  engine,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await openMap(page, await engine.loginUrl());
  const light = await root(page);
  expect(light.attr).toBeNull();
  expect(light.scheme).toBe("light");

  await page.emulateMedia({ colorScheme: "dark" });
  const dark = await root(page);
  expect(dark.attr).toBeNull();
  expect(dark.scheme).toBe("dark");
  expect(dark.bg).not.toBe(light.bg);
});

test("a_pin_wins_over_the_system_and_applies_before_first_paint", async ({
  page,
  engine,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  // The lens listing answers only once the sheet is open: the focus that fell back to the theme
  // choice moves to the current lens when the lenses arrive.
  let answer = () => {};
  const listed = new Promise<void>((resolve) => (answer = resolve));
  await page.route("**/api/v1/ask", async (route) => {
    await listed;
    await route.continue();
  });
  await openMap(page, await engine.loginUrl());

  // Keyboard first: Enter opens the sheet on the current lens, Tab reaches the checked choice,
  // arrows only move focus, and Escape gives the focus back to the switcher.
  const sheet = page.locator(".picker-sheet");
  await switcher(page).focus();
  await page.keyboard.press("Enter");
  await expect(sheet).toBeVisible();
  answer();
  await expect(sheet.locator(".picker-entry[aria-current]")).toBeFocused();
  await page.unroute("**/api/v1/ask");
  // Each lens entry after the current one is a stop of its own before the theme choice.
  const entries = await sheet.locator(".picker-entry").all();
  const at = await sheet
    .locator(".picker-entry")
    .evaluateAll((els) => els.findIndex((e) => e.hasAttribute("aria-current")));
  for (let i = at; i < entries.length; i++) {
    await page.keyboard.press("Tab");
  }
  await expect(
    page.getByRole("radio", { name: CANVAS_STRINGS.theme_system }),
  ).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("radio", { name: CANVAS_STRINGS.theme_light }),
  ).toBeFocused();
  await expect(
    page.getByRole("radio", { name: CANVAS_STRINGS.theme_system }),
  ).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);
  await expect(switcher(page)).toBeFocused();

  await switcher(page).click();
  await page.getByRole("radio", { name: CANVAS_STRINGS.theme_dark }).click();
  await expect(sheet).toHaveCount(0);
  expect(await root(page)).toMatchObject({ attr: "dark", scheme: "dark" });

  // The script in the head runs before the bundle: with the bundle emptied, the pin still shows.
  await page.route("**/assets/index.js", (r) =>
    r.fulfill({ status: 200, contentType: "text/javascript", body: "" }),
  );
  await page.reload({ waitUntil: "domcontentloaded" });
  expect((await root(page)).attr).toBe("dark");
  expect(
    await page.evaluate(() =>
      document.head.firstElementChild?.getAttribute("src"),
    ),
  ).toBe("./assets/theme.js");
  await page.unroute("**/assets/index.js");
  await page.reload();
  await page.locator(".map[data-ready]").waitFor();
  expect((await root(page)).attr).toBe("dark");

  await switcher(page).click();
  await page.getByRole("radio", { name: CANVAS_STRINGS.theme_system }).click();
  expect(await root(page)).toMatchObject({ attr: null, scheme: "light" });
});

test("a_choice_in_one_tab_reaches_another", async ({ page, engine }) => {
  await openMap(page, await engine.loginUrl());
  const second = await page.context().newPage();
  await openMap(second, await engine.loginUrl());

  await switcher(page).click();
  await page.getByRole("radio", { name: CANVAS_STRINGS.theme_dark }).click();
  await expect
    .poll(() =>
      second.evaluate(() =>
        document.documentElement.getAttribute("data-theme"),
      ),
    )
    .toBe("dark");
});

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("the_theme_choice_is_three_44px_radios_on_the_phone", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    expect(
      await page.locator(".top-row").locator("button, input").count(),
    ).toBeLessThanOrEqual(4);
    await switcher(page).tap();
    const radios = page.getByRole("radio");
    await expect(radios).toHaveCount(3);
    const box = (await page.locator(".picker-sheet").boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(PHONE.width);
    for (const radio of await radios.all()) {
      await expect(radio).toBeVisible();
      const hit = await hitSize(radio);
      expect(hit.width).toBeGreaterThanOrEqual(44);
      expect(hit.height).toBeGreaterThanOrEqual(44);
    }
  });
});
