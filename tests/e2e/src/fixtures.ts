/**
 * The specs' shared fixtures: the real engine per test (a scanned copy of the fixture and
 * `planroom serve` on it), the requests the browser sent, and a check that no page logged a
 * console error or threw.
 */

import {
  test as base,
  expect,
  type BrowserContext,
  type Page,
} from "@playwright/test";

import { startEngine, type Engine } from "./engine";

export const DESKTOP = { width: 1440, height: 900 } as const;
export const PHONE = { width: 390, height: 844 } as const;

/** The five concepts, the only top-level controls (R14, DR-77). */
export const CONCEPTS = ["decisions", "diffs", "intent", "lenses", "levels"];

/** One request the browser sent, with the `Origin` it carried. */
export interface Sent {
  method: string;
  path: string;
  origin: string | undefined;
}

/** Every request `context`'s pages send from now on, in order. */
export function record(context: BrowserContext): Sent[] {
  const sent: Sent[] = [];
  context.on("request", (r) => {
    const entry: Sent = {
      method: r.method(),
      path: new URL(r.url()).pathname,
      origin: undefined,
    };
    sent.push(entry);
    void r
      .allHeaders()
      .then((h) => {
        entry.origin = h.origin;
      })
      .catch(() => undefined);
  });
  return sent;
}

export const test = base.extend<{
  engine: Engine;
  sent: Sent[];
  consoleErrors: string[];
}>({
  // eslint-disable-next-line no-empty-pattern -- Playwright reads a fixture's dependencies from this pattern.
  engine: async ({}, use) => {
    const engine = await startEngine();
    await use(engine);
    engine.close();
  },
  sent: async ({ context }, use) => {
    await use(record(context));
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

/** Sign in with `url`, which carries a fresh code, and wait for the map to be interactive. */
export async function openMap(page: Page, url: string): Promise<void> {
  await page.goto(url);
  await page.locator(".map[data-ready]").waitFor();
}
