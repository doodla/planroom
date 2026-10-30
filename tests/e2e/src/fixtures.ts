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

import {
  CANVAS_STRINGS,
  type AskItem,
  type AskResult,
} from "@planroom/contract";

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

/**
 * The phone's top row (design-language.md, "Touch"): four targets at most, the part of each a
 * press reaches at least 44 px both ways, every visible target's box too, and no overflow.
 */
export async function topRowHolds(page: Page): Promise<void> {
  const row = page.locator(".top-row");
  expect(await row.locator("button, input").count()).toBeLessThanOrEqual(4);
  const rowBox = (await row.boundingBox())!;
  expect(rowBox.height).toBeLessThan(70);
  for (const target of await row
    .locator("button:visible, input:visible")
    .all()) {
    const hit = await hitSize(target);
    const what = await target.evaluate((e) => e.outerHTML.slice(0, 80));
    expect(hit.width, `reachable width of ${what}`).toBeGreaterThanOrEqual(44);
    expect(hit.height, `reachable height of ${what}`).toBeGreaterThanOrEqual(
      44,
    );
  }
  for (const target of await page
    .locator("button:visible, input:visible")
    .all()) {
    const box = (await target.boundingBox())!;
    const what = await target.evaluate((e) => e.outerHTML.slice(0, 80));
    expect(box.width, what).toBeGreaterThanOrEqual(44);
    expect(box.height, what).toBeGreaterThanOrEqual(44);
  }
  const widths = await page.evaluate(() => [
    document.documentElement.scrollWidth,
    document.body.scrollWidth,
  ]);
  expect(Math.max(...widths)).toBeLessThanOrEqual(390);
}

/**
 * The lens items of the engine's `ask` listing, as the lens switcher lists them: an item whose one
 * handle is a `map` naming a lens. The CLI pages its replies, so `meta.next` is followed.
 */
export function lensItems(engine: Engine): AskItem[] {
  const items: AskItem[] = [];
  let cursor: string | null = null;
  do {
    const reply = engine.cli(
      cursor === null ? ["ask"] : ["ask", "--cursor", cursor],
    );
    items.push(...(reply.result as AskResult).items);
    cursor = reply.meta.next ?? null;
  } while (cursor !== null);
  return items.filter(
    (i) =>
      i.handles.length === 1 &&
      i.handles[0]!.command === "map" &&
      i.handles[0]!.args.includes("--lens"),
  );
}

/** What `token` (such as `yarn-teal`) resolves to on the page, as a computed color. */
export function tokenColor(
  page: Page,
  token: string,
  property: "color" | "backgroundColor" = "color",
): Promise<string> {
  return page.evaluate(
    ([name, prop]) => {
      const probe = document.createElement("div");
      probe.style[prop as "color"] = `var(--${name})`;
      document.body.append(probe);
      const color = getComputedStyle(probe)[prop as "color"];
      probe.remove();
      return color;
    },
    [token, property] as const,
  );
}

/**
 * Two presses on `target` within the dive's window: a double click, or on the phone two taps at
 * its middle, sent back to back.
 */
export async function doublePress(
  page: Page,
  target: Locator,
  phone: boolean,
): Promise<void> {
  if (!phone) {
    await target.dblclick();
    return;
  }
  const box = (await target.boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.touchscreen.tap(x, y);
  await page.touchscreen.tap(x, y);
}

/**
 * One press on a lifted edge, a third of the way along its path so the count's label, at the
 * middle, isn't what takes it: a click, or on the phone a tap.
 */
export async function pressEdge(
  page: Page,
  edge: Locator,
  phone: boolean,
): Promise<void> {
  const at = await edge
    .locator("path.react-flow__edge-interaction")
    .evaluate((el) => {
      const path = el as unknown as SVGPathElement;
      const p = path.getPointAtLength(path.getTotalLength() / 3);
      const m = path.getScreenCTM()!;
      return { x: m.a * p.x + m.c * p.y + m.e, y: m.b * p.x + m.d * p.y + m.f };
    });
  if (phone) {
    await page.touchscreen.tap(at.x, at.y);
  } else {
    await page.mouse.click(at.x, at.y);
  }
}

/** The IDs of the blocks the page draws, sorted. */
export function drawnIds(page: Page): Promise<string[]> {
  return page
    .locator(".react-flow__node")
    .evaluateAll((els) => els.map((e) => e.getAttribute("data-id")!).sort());
}

/**
 * The concepts of the page as drawn now, after checking that every visible button and input sits
 * inside one (R14, DR-77): the controls that don't are named in the failure.
 */
export async function conceptsOf(page: Page): Promise<string[]> {
  const found = await page.evaluate(() => {
    const visible = (el: Element) => {
      const style = getComputedStyle(el);
      return el.getClientRects().length > 0 && style.visibility !== "hidden";
    };
    const loose = [...document.querySelectorAll("button, input")]
      .filter((el) => visible(el) && el.closest("[data-concept]") === null)
      .map((el) => el.outerHTML.slice(0, 100));
    const concepts = [
      ...new Set(
        [...document.querySelectorAll("[data-concept]")].map((el) =>
          el.getAttribute("data-concept"),
        ),
      ),
    ].sort();
    return { loose, concepts: concepts as string[] };
  });
  expect(found.loose, "controls outside every concept").toEqual([]);
  return found.concepts;
}

/**
 * Go to the level labelled `label` the way the form factor does: the desktop's rail, the phone's
 * `⋯` sheet.
 */
export async function goToLevel(
  page: Page,
  label: string,
  phone: boolean,
): Promise<void> {
  if (phone) {
    await page.getByRole("button", { name: CANVAS_STRINGS.levels_sheet }).tap();
    await page
      .getByRole("dialog", { name: CANVAS_STRINGS.levels_sheet })
      .getByRole("button", { name: label })
      .tap();
  } else {
    await page
      .locator(".rail")
      .getByRole("button", { name: label, exact: true })
      .click();
  }
}
