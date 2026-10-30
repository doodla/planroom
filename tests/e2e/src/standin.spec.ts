/**
 * The UI states the engine can't produce yet, against the stand-in (`standin.ts`): the real
 * engine's golden outputs with a test-only overlay from `tests/e2e/overlays/` laid over them. Each
 * test names the milestone that makes its state real; when the engine produces it, the test moves
 * to a spec that runs on the real engine, such as `surface.spec.ts` or `first-look.spec.ts`, and
 * its overlay goes.
 */

import { mergeTests, test as base } from "@playwright/test";
import { CANVAS_STRINGS } from "@planroom/contract";

import { DESKTOP, PHONE, expect, openMap, test } from "./fixtures";
import { standIn, type StandIn } from "./standin";

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

// Real in M2, when the decision runner opens cards and the diff engine finds diffs.
test.describe("M2 stand-in: a decision card and diffs", () => {
  const t = mergeTests(test, withStandIn([M2_CARD]));

  t.describe("at 1440x900", () => {
    t.use({ viewport: DESKTOP });

    t(
      "the_card_shows_its_choices_as_labels_with_the_preselected_one_marked",
      async ({ page, standin }) => {
        await openMap(page, standin.loginUrl());
        const card = page.locator("[data-concept=decisions] .face-up");
        await expect(card.getByRole("heading")).toHaveText(
          "Apply Claude's 2 edits to the worker group?",
        );
        // `decide` is no command in M1: the pane picks, so a choice is a label, not a button.
        await expect(card.getByRole("button")).toHaveCount(0);
        await expect(
          card.locator(".card-choice[aria-current=true]"),
        ).toHaveText("Accept both");
        await expect(card.locator(".card-choice")).toHaveText([
          "Accept both",
          "Only the first",
          "Reject",
          "Later",
        ]);
        await expect(card).toContainText(CANVAS_STRINGS.card_in_pane);
        expect(standin.runs.map((r) => r.path)).not.toContain("/api/v1/decide");
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
          sheet.locator(".card-choice[aria-current=true]"),
        ).toHaveText("Accept both");
        await sheet.locator(".tray-badge").tap();
        await expect(sheet).toHaveAttribute("data-detent", "half");
        await expect(sheet.locator(".tray-row")).toHaveCount(2);
      },
    );
  });
});
