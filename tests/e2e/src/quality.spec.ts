/**
 * S13's bar (DR-43): the canvas is interactive in under 1 s on every one of 5 cold loads, each in
 * a fresh browser context, with every value recorded. Interactive is the latest of first
 * contentful paint, the map's `planroom:interactive` mark (every block measured) and the end of
 * the last long task.
 */

import type { Page } from "@playwright/test";

import { DESKTOP, expect, test } from "./fixtures";

const BAR_MS = 1000;

async function interactiveAt(page: Page): Promise<number> {
  await page.locator(".map[data-ready]").waitFor();
  // A quiet window, so a long task right after the mark still counts.
  await page.waitForTimeout(500);
  return page.evaluate(() => {
    const ready =
      performance.getEntriesByName("planroom:interactive")[0]?.startTime ??
      Infinity;
    const fcp =
      performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? 0;
    const longTasks = (window as unknown as { __longTaskEnds: number[] })
      .__longTaskEnds;
    return Math.round(Math.max(ready, fcp, ...longTasks));
  });
}

async function coldLoad(page: Page, url: string): Promise<number> {
  await page.addInitScript(() => {
    const ends: number[] = [];
    (window as unknown as { __longTaskEnds: number[] }).__longTaskEnds = ends;
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        ends.push(e.startTime + e.duration);
      }
    }).observe({ type: "longtask", buffered: true });
  });
  await page.goto(url);
  return interactiveAt(page);
}

test("interactive_in_under_1s_on_each_of_5_cold_loads", async ({
  browser,
  served,
}) => {
  // The first look lays the structure out in the worker and stores it, as `serve` then keeps it.
  const first = await browser.newContext({ viewport: DESKTOP });
  const layoutLoad = await coldLoad(await first.newPage(), served.loginUrl());
  await expect.poll(() => served.layouts.length).toBe(1);
  await first.close();

  const samples: number[] = [];
  for (let i = 0; i < 5; i++) {
    const context = await browser.newContext({ viewport: DESKTOP });
    samples.push(await coldLoad(await context.newPage(), served.loginUrl()));
    await context.close();
  }
  const line = `interactive (ms), 5 cold loads with stored positions: ${samples.join(", ")}; first look with the ELK layout: ${layoutLoad}`;
  console.log(line);
  test.info().annotations.push({ type: "interactive", description: line });
  for (const sample of samples) {
    expect(sample).toBeLessThan(BAR_MS);
  }
});
