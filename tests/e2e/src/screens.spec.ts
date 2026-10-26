/**
 * The walking skeleton's screenshots, written only by `cargo xtask e2e --screens`: the main view
 * and the empty state at 1440x900 and 390x844, in both themes, into docs/screenshots/.
 */

import { join } from "node:path";

import { DESKTOP, PHONE, expect, test } from "./fixtures";
import { ROOT, serve } from "./server";

const OUT = join(ROOT, "docs/screenshots");

const VIEWS = [
  { name: "main", map: "map-graph.json", ready: ".map[data-ready]" },
  { name: "empty", map: "map-empty.json", ready: "[data-empty] img" },
] as const;

test.skip(
  process.env.PLANROOM_SCREENS !== "1",
  "screenshots are written by cargo xtask e2e --screens",
);

for (const view of VIEWS) {
  for (const viewport of [DESKTOP, PHONE]) {
    for (const colorScheme of ["light", "dark"] as const) {
      const file = `${view.name}-${viewport.width}x${viewport.height}-${colorScheme}.png`;
      test(file, async ({ browser }) => {
        const served = await serve({ map: view.map });
        const phone = viewport === PHONE;
        const context = await browser.newContext({
          viewport,
          colorScheme,
          deviceScaleFactor: phone ? 2 : 1,
          hasTouch: phone,
          isMobile: phone,
        });
        const page = await context.newPage();
        const errors: string[] = [];
        page.on(
          "console",
          (m) => m.type() === "error" && errors.push(m.text()),
        );
        await page.goto(served.loginUrl());
        await page.locator(view.ready).waitFor();
        await page.evaluate(() => document.fonts.ready.then(() => undefined));
        await page.waitForTimeout(300);
        await page.screenshot({ path: join(OUT, file) });
        expect(errors).toEqual([]);
        await context.close();
        await served.close();
      });
    }
  }
}
