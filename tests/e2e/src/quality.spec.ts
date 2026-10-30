/**
 * S13's bar (DR-43), against the real `planroom serve` on the scanned fixture: the canvas is
 * interactive in under 1 s on every one of 5 cold loads, each in a fresh browser context, with
 * every value recorded. Interactive is the latest of first contentful paint, the map's
 * `planroom:interactive` mark (every block measured) and the end of the last long task.
 */

import { CANVAS_STRINGS, type MapView } from "@planroom/contract";
import type { Page } from "@playwright/test";

import {
  DESKTOP,
  PHONE,
  expect,
  lensItems,
  openMap,
  test,
  topRowHolds,
} from "./fixtures";

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

test("interactive_under_1_s", async ({ browser, engine }) => {
  // The first look lays the structure out in the worker and stores it, as `serve` then keeps it.
  const first = await browser.newContext({ viewport: DESKTOP });
  const page = await first.newPage();
  const stored = page.waitForResponse(
    (r) => r.request().method() === "PUT" && r.url().endsWith("/api/v1/layout"),
  );
  const layoutLoad = await coldLoad(page, await engine.loginUrl());
  expect((await stored).status()).toBe(204);
  await first.close();

  const samples: number[] = [];
  for (let i = 0; i < 5; i++) {
    const context = await browser.newContext({ viewport: DESKTOP });
    samples.push(
      await coldLoad(await context.newPage(), await engine.loginUrl()),
    );
    await context.close();
  }
  const line = `interactive (ms), 5 cold loads with stored positions: ${samples.join(", ")}; first look with the ELK layout: ${layoutLoad}`;
  console.log(line);
  test.info().annotations.push({ type: "interactive", description: line });
  for (const sample of samples) {
    expect(sample).toBeLessThan(BAR_MS);
  }
});

/**
 * Every project chip and level name on the top row either shows whole or ellipsizes inside its
 * box with the whole text in a `title`, and the page never scrolls sideways.
 */
async function namesFit(page: Page): Promise<void> {
  const names = await page
    .locator(".top-row .chip-label, .top-row .lid-name")
    .evaluateAll((els) =>
      els.map((e) => ({
        text: e.textContent ?? "",
        title: e.closest("[title]")?.getAttribute("title") ?? null,
        over: e.scrollWidth > e.clientWidth,
        overflow: getComputedStyle(e).textOverflow,
      })),
    );
  for (const name of names) {
    if (name.over) {
      expect(name.overflow, `${name.text} ellipsizes`).toBe("ellipsis");
      expect(name.title, `${name.text} carries its whole text`).toBe(name.text);
    }
  }
  const widths = await page.evaluate(() => [
    document.documentElement.scrollWidth,
    document.body.scrollWidth,
  ]);
  expect(Math.max(...widths)).toBeLessThanOrEqual(PHONE.width);
}

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("top_row_fits_at_the_deepest_level", async ({ page, engine }) => {
    const levels = (engine.cli(["map", "--detail", "graph"]).result as MapView)
      .levels!;
    const lid = page.locator(".breadcrumb .lid-name");
    const goTo = async (label: string, shown = label) => {
      await page
        .getByRole("button", { name: CANVAS_STRINGS.levels_sheet })
        .tap();
      await page
        .getByRole("dialog", { name: CANVAS_STRINGS.levels_sheet })
        .getByRole("button", { name: label, exact: true })
        // The sheet lists the lids above before the levels, and a lid may share a level's label.
        .last()
        .tap();
      await expect(lid).toHaveText(shown);
      await page.locator(".map[data-ready]").waitFor();
    };

    // Each served level shows whole in the current lid, from the top down to the finest; the
    // top's lid names the workspace rather than its level.
    const served = levels.filter((l) => !l.apart);
    let top = "";
    const eachLevelShowsWhole = async (picked: string) => {
      for (const level of served) {
        if (level === served[0]) {
          top ||= (await lid.textContent()) ?? "";
          if ((await lid.textContent()) !== top) {
            await goTo(level.label, top);
          }
        } else {
          await goTo(level.label);
        }
        const [scroll, client] = await lid.evaluate((e) => [
          e.scrollWidth,
          e.clientWidth,
        ]);
        expect(
          scroll,
          `${level.label} shows whole with ${picked}`,
        ).toBeLessThanOrEqual(client);
      }
    };
    await openMap(page, await engine.loginUrl());
    await eachLevelShowsWhole("no project picked");

    // With one project of a 60-character name, the shelf holds it as the current project and
    // each level still shows whole beside it.
    const names = ["a", "b", "c"].map((n) =>
      `${n} ${"long project name ".repeat(4)}`.slice(0, 60),
    );
    const make = (name: string) =>
      engine.cli(["intent", "new", name], "agent:e2e:tester");
    make(names[0]!);
    await page.goto(await engine.loginUrl());
    await page.locator(".map[data-ready]").waitFor();
    await expect(page.locator(".top-row [data-concept=intent]")).toContainText(
      names[0]!,
    );
    await eachLevelShowsWhole("a 60-character project");

    // Three projects with long names, the finest level, every lens, search open.
    make(names[1]!);
    make(names[2]!);
    await page.goto(await engine.loginUrl());
    await page.locator(".map[data-ready]").waitFor();
    await goTo(served.at(-1)!.label);
    for (const item of lensItems(engine)) {
      await page.getByRole("button", { name: CANVAS_STRINGS.lenses }).tap();
      await page
        .locator(".picker-sheet")
        .getByRole("button", { name: item.name, exact: true })
        .tap();
      await page.locator(".map[data-ready]").waitFor();
      await topRowHolds(page);
      await namesFit(page);
      await page.getByRole("button", { name: CANVAS_STRINGS.search }).tap();
      await topRowHolds(page);
      await namesFit(page);
      await page
        .getByRole("button", { name: CANVAS_STRINGS.close_search })
        .tap();
    }
  });
});
