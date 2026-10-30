/**
 * The walking skeleton's screenshots, written only by `cargo xtask e2e --screens`, which alone
 * runs the `screens` project this file is in (playwright.config.ts), at 1440x900
 * and 390x844, in both themes, into docs/screenshots/: the main view from the real
 * `planroom serve` on the scanned fixture, and the empty state from the real `planroom serve` on
 * the fixture before its first scan; the unborn one is a workspace of repos with no commit. Two more show
 * the way back up (ux-4) and a level with no blocks (ux-3): the container level the next action
 * opens, its `⋯` lid open on the phone; the lens switcher's sheet open; the intent level, which the
 * desktop's rail reaches; the phone's `⋯` sheet of levels; a block gone into with a double press; and an edge's evidence sheet.
 */

import { join } from "node:path";

import type { Page } from "@playwright/test";
import { CANVAS_STRINGS } from "@planroom/contract";

import { initRepo, removeRepos, startEngine } from "./engine";
import {
  DESKTOP,
  PHONE,
  doublePress,
  expect,
  pressEdge,
  test,
} from "./fixtures";
import { ROOT } from "./standin";

const OUT = join(ROOT, "docs/screenshots");

interface View {
  name: string;
  scanned: boolean;
  /** Changes the workspace before the engine serves it. */
  prepare?: (root: string) => void;
  ready: string;
  /** Where the view goes after the map is ready, on each form factor. */
  go?: (page: Page, phone: boolean) => Promise<void>;
  /**
   * Which form factor shows it: false for the desktop alone (the intent level is the rail's), true
   * for the phone alone (the `⋯` sheet); unset for both.
   */
  phone?: boolean;
}

const VIEWS: readonly View[] = [
  { name: "main", scanned: true, ready: ".map[data-ready]" },
  { name: "empty", scanned: false, ready: "[data-empty] img" },
  {
    name: "unborn",
    scanned: false,
    prepare: (root) => {
      removeRepos(root);
      initRepo(root, "alpha");
      initRepo(root, "beta");
    },
    ready: "[data-empty] img",
  },
  {
    name: "container",
    scanned: true,
    ready: ".map[data-ready]",
    go: async (page, phone) => {
      await page
        .locator(phone ? ".sheet" : "[data-concept=decisions]")
        .getByRole("button", { name: "Open the containers" })
        .first()
        .click();
      await page
        .locator(".breadcrumb .lid-name", { hasText: "container" })
        .waitFor();
      await page.locator(".map[data-ready]").waitFor();
      if (phone) {
        await page
          .getByRole("button", { name: CANVAS_STRINGS.levels_sheet })
          .click();
      }
    },
  },
  {
    name: "levels-sheet",
    scanned: true,
    ready: ".map[data-ready]",
    phone: true,
    go: async (page) => {
      await page
        .getByRole("button", { name: CANVAS_STRINGS.levels_sheet })
        .click();
      await page
        .getByRole("dialog", { name: CANVAS_STRINGS.levels_sheet })
        .waitFor();
    },
  },
  {
    name: "lens-sheet",
    scanned: true,
    ready: ".map[data-ready]",
    go: async (page) => {
      await page.getByRole("button", { name: CANVAS_STRINGS.lenses }).click();
      await page.getByRole("dialog", { name: CANVAS_STRINGS.lenses }).waitFor();
    },
  },
  {
    name: "dive",
    scanned: true,
    ready: ".map[data-ready]",
    go: async (page, phone) => {
      // The first block with containers in it, the top map's first.
      const first = page.locator(".react-flow__node", {
        has: page.locator(".block-count"),
      });
      await doublePress(page, first.first(), phone);
      await expect(page.locator(".breadcrumb .lid-name")).not.toHaveText(
        "shop",
      );
      await page.locator(".map[data-ready]").waitFor();
    },
  },
  {
    name: "evidence",
    scanned: true,
    ready: ".map[data-ready]",
    go: async (page, phone) => {
      await pressEdge(page, page.locator(".react-flow__edge").first(), phone);
      await page
        .getByRole("dialog", { name: CANVAS_STRINGS.evidence })
        .waitFor();
    },
  },
  {
    name: "empty-level",
    scanned: true,
    ready: ".map[data-ready]",
    phone: false,
    go: async (page) => {
      await page
        .locator(".rail")
        .getByRole("button", { name: "intent", exact: true })
        .click();
      await page.locator(".level-empty [data-empty] img").waitFor();
    },
  },
];

for (const view of VIEWS) {
  const sizes =
    view.phone === undefined
      ? [DESKTOP, PHONE]
      : [view.phone ? PHONE : DESKTOP];
  for (const viewport of sizes) {
    for (const colorScheme of ["light", "dark"] as const) {
      const file = `${view.name}-${viewport.width}x${viewport.height}-${colorScheme}.png`;
      test(file, async ({ browser }) => {
        const server = await startEngine({
          scanned: view.scanned,
          ...(view.prepare ? { prepare: view.prepare } : {}),
        });
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
          await view.go?.(page, phone);
          await page.evaluate(() => document.fonts.ready.then(() => undefined));
          await page.waitForTimeout(300);
          await page.screenshot({ path: join(OUT, file) });
          expect(errors).toEqual([]);
        } finally {
          await context.close();
          server.close();
        }
      });
    }
  }
}
