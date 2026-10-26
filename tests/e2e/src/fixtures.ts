/**
 * The specs' shared fixtures: a stand-in server per test, and a check that no page logged a
 * console error or threw.
 */

import { test as base, expect, type Page } from "@playwright/test";

import { serve, type Served } from "./server";

export const DESKTOP = { width: 1440, height: 900 } as const;
export const PHONE = { width: 390, height: 844 } as const;

/** The five concepts, the only top-level controls (R14, DR-77). */
export const CONCEPTS = ["decisions", "diffs", "intent", "lenses", "levels"];

export const test = base.extend<{ served: Served; consoleErrors: string[] }>({
  // eslint-disable-next-line no-empty-pattern -- Playwright reads a fixture's dependencies from this pattern.
  served: async ({}, use) => {
    const served = await serve();
    await use(served);
    await served.close();
  },
  consoleErrors: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on("console", (m) => {
        if (m.type() === "error") {
          errors.push(m.text());
        }
      });
      page.on("pageerror", (e) => errors.push(e.message));
      await use(errors);
      expect(errors, "console errors").toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

/** Sign in with a fresh code and wait for the map to be interactive. */
export async function openMap(page: Page, served: Served): Promise<void> {
  await page.goto(served.loginUrl());
  await page.locator(".map[data-ready]").waitFor();
}
