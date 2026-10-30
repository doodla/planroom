/**
 * The specs' shared fixtures: the real engine per test (a scanned copy of the fixture and
 * `planroom serve` on it), the requests the browser sent, and a check that no page logged a
 * console error or threw.
 */

import {
  test as base,
  expect,
  type BrowserContext,
  type Locator,
  type Page,
  type Request,
} from "@playwright/test";

import { startEngine, type Engine } from "./engine";

export const DESKTOP = { width: 1440, height: 900 } as const;
export const PHONE = { width: 390, height: 844 } as const;

/** The five concepts, the only top-level controls (R14, DR-77). */
export const CONCEPTS = ["decisions", "diffs", "intent", "lenses", "levels"];

/** One request the browser sent, with the `Origin` it carried and whether it has ended. */
export interface Sent {
  method: string;
  path: string;
  origin: string | undefined;
  /** It got its response, or failed; until then its headers may not be known yet. */
  done: boolean;
}

/** Every request `context`'s pages send from now on, in order. */
export function record(context: BrowserContext): Sent[] {
  const sent: Sent[] = [];
  const entries = new WeakMap<Request, Sent>();
  context.on("request", (r) => {
    const entry: Sent = {
      method: r.method(),
      path: new URL(r.url()).pathname,
      origin: undefined,
      done: false,
    };
    entries.set(r, entry);
    sent.push(entry);
    void r
      .allHeaders()
      .then((h) => {
        entry.origin = h.origin;
      })
      .catch(() => undefined);
  });
  const end = (r: Request) => {
    const entry = entries.get(r);
    if (entry) {
      entry.done = true;
    }
  };
  context.on("requestfinished", end);
  context.on("requestfailed", end);
  return sent;
}

/** Whether every request but the live stream's, which never ends, has ended. */
export function settled(sent: Sent[]): boolean {
  return sent.every((r) => r.done || r.path === "/api/v1/stream");
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

/**
 * How much of `target` a press reaches, in CSS px: `document.elementFromPoint` sampled once a
 * pixel across its middle row, then down the middle of what that row hit. A neighbour that paints
 * over part of a target takes those hits, which its bounding box never shows.
 */
export async function hitSize(
  target: Locator,
): Promise<{ width: number; height: number }> {
  return target.evaluate((el) => {
    const r = el.getBoundingClientRect();
    const owns = (x: number, y: number) => {
      const hit = document.elementFromPoint(x, y);
      return hit !== null && el.contains(hit);
    };
    // Whole pixels: Chromium rounds a fractional point before it hit-tests.
    const y = Math.round(r.top + r.height / 2);
    const xs: number[] = [];
    for (let x = Math.ceil(r.left); x < r.right; x++) {
      if (owns(x, y)) {
        xs.push(x);
      }
    }
    if (xs.length === 0) {
      return { width: 0, height: 0 };
    }
    const x = xs[Math.floor(xs.length / 2)]!;
    let height = 0;
    for (let yy = Math.ceil(r.top); yy < r.bottom; yy++) {
      if (owns(x, yy)) {
        height++;
      }
    }
    return { width: xs.length, height };
  });
}
