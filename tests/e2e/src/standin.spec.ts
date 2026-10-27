/**
 * The UI states the engine can't produce yet, against the stand-in (`standin.ts`): the real
 * engine's golden outputs with a test-only overlay from `tests/e2e/overlays/` laid over them. Each
 * test names the milestone that makes its state real; when the engine produces it, the test moves
 * to `surface.spec.ts` and its overlay goes.
 */

import { mergeTests, test as base } from "@playwright/test";

import { DESKTOP, PHONE, expect, openMap, test } from "./fixtures";
import { standIn, type StandIn } from "./standin";

/** The M1 overlay: kind tones (DR-46) and lifted edges over the real fixture's blocks. */
const M1_TONES = "m1-tones-and-edges.map.json";
/** The M2 overlay: one face-up decision card and two diff lines in the real pane. */
const M2_CARD = "m2-decision-and-diffs.focus.json";

const withStandIn = (overlays: readonly string[]) =>
  base.extend<{ standin: StandIn }>({
    // eslint-disable-next-line no-empty-pattern -- Playwright reads a fixture's dependencies from this pattern.
    standin: async ({}, use) => {
      const s = await standIn({ overlays });
      await use(s);
      await s.close();
    },
  });

// Real in M1, when packs declare kind tones and the linkers lift relations.
test.describe("M1 stand-in: tones and lifted edges", () => {
  const t = mergeTests(test, withStandIn([M1_TONES]));
  t.use({ viewport: DESKTOP });

  t(
    "the_blocks_take_their_kinds_tones_and_the_lifted_edges_their_counts",
    async ({ page, standin }) => {
      await openMap(page, standin.loginUrl());
      const blocks = page.locator(".block");
      await expect(blocks).toHaveCount(10);
      const tones = await blocks.evaluateAll((els) =>
        els.map((e) => e.getAttribute("data-tone")),
      );
      expect(new Set(tones)).toEqual(
        new Set(["service", "library", "store", "infra"]),
      );
      // A toned block fills with its tone's token and keeps its kind label.
      const orders = page.locator(".block", { hasText: /^orders\s*repo/ });
      expect(
        await orders.evaluate((e) => {
          const probe = document.createElement("div");
          probe.style.background = "var(--block-service)";
          document.body.append(probe);
          const fill = getComputedStyle(probe).backgroundColor;
          probe.remove();
          return getComputedStyle(e).backgroundColor === fill;
        }),
      ).toBe(true);
      await expect(orders.locator(".block-kind")).toHaveText("repo");
      await expect(page.locator(".react-flow__edge")).toHaveCount(8);
      await expect(page.locator(".react-flow__edge-text")).toHaveText([
        "3",
        "2",
        "2",
        "2",
      ]);
      await expect(
        page.getByRole("treeitem", { name: "http → orders, 3" }),
      ).toBeAttached();
    },
  );
});

// Real in M2, when the decision runner opens cards and the diff engine finds diffs.
test.describe("M2 stand-in: a decision card and diffs", () => {
  const t = mergeTests(test, withStandIn([M2_CARD]));

  t.describe("at 1440x900", () => {
    t.use({ viewport: DESKTOP });

    t(
      "the_card_and_its_preselected_pick_take_one_tap",
      async ({ page, standin }) => {
        await openMap(page, standin.loginUrl());
        const card = page.locator("[data-concept=decisions] .face-up");
        await expect(card.getByRole("heading")).toHaveText(
          "Apply Claude's 2 edits to the worker group?",
        );
        const accept = card.getByRole("button", { name: "Accept both" });
        await expect(accept).toHaveAttribute("aria-pressed", "true");
        await accept.click();
        await expect
          .poll(() => standin.runs.map((r) => r.path))
          .toContain("/api/v1/decide");
        expect(standin.runs.at(-1)?.body).toEqual({
          id: "card_01JB7Q",
          choice: "accept",
        });
      },
    );

    t(
      "the_tray_badge_counts_the_diffs_and_opens_the_tray",
      async ({ page, standin }) => {
        await openMap(page, standin.loginUrl());
        const badge = page.locator("[data-concept=diffs]");
        await expect(badge).toContainText("2");
        await badge.click();
        await expect(page.locator(".tray-row")).toHaveText([
          "worker → platform: + calls orders-api",
          "storefront: ~ checkout moved to cart",
        ]);
      },
    );
  });

  t.describe("at 390x844 with touch", () => {
    t.use({ viewport: PHONE, hasTouch: true, isMobile: true });

    t(
      "the_sheet_peeks_with_the_card_and_opens_to_half_with_the_tray",
      async ({ page, standin }) => {
        await openMap(page, standin.loginUrl());
        const sheet = page.locator(".sheet");
        await expect(sheet).toHaveAttribute("data-detent", "peek");
        await expect(
          sheet.getByRole("button", { name: "Accept both" }),
        ).toBeVisible();
        await sheet.locator("[data-concept=diffs]").tap();
        await expect(sheet).toHaveAttribute("data-detent", "half");
        await expect(sheet.locator(".tray-row")).toHaveCount(2);
      },
    );
  });
});
