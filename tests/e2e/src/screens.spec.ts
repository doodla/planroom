/**
 * The walking skeleton's screenshots, written only by `cargo xtask e2e --screens`, at 1440x900
 * and 390x844, in both themes, into docs/screenshots/: the main view from the real
 * `planroom serve` on the scanned fixture, and the empty state from the stand-in's real
 * `map-empty.json`, since `serve` can't start in an unscanned workspace yet (M1).
 */

import { join } from "node:path";

import { startEngine } from "./engine";
import { DESKTOP, PHONE, expect, test } from "./fixtures";
import { ROOT, standIn } from "./standin";

const OUT = join(ROOT, "docs/screenshots");

const VIEWS = [
  { name: "main", scanned: true, ready: ".map[data-ready]" },
  { name: "empty", scanned: false, ready: "[data-empty] img" },
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
        const server = view.scanned
          ? await startEngine()
          : await standIn({ map: "map-empty.json" });
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
        try {
          await page.goto(await server.loginUrl());
          await page.locator(view.ready).waitFor();
          await page.evaluate(() => document.fonts.ready.then(() => undefined));
          await page.waitForTimeout(300);
          await page.screenshot({ path: join(OUT, file) });
          expect(errors).toEqual([]);
        } finally {
          await context.close();
          await server.close();
        }
      });
    }
  }
}
