/**
 * The canvas against the stand-in server: the five concepts, the map from `map --detail graph`,
 * the ELK layout and its write-back, the login, the empty states, live refetch, search, the phone
 * layout and reduced motion. Every test also fails on a console error.
 */

import { CONCEPTS, DESKTOP, PHONE, expect, openMap, test } from "./fixtures";
import { serve } from "./server";

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("exactly_five_concepts", async ({ page, served }) => {
    await openMap(page, served);
    const concepts = await page
      .locator("[data-concept]")
      .evaluateAll((els) => [
        ...new Set(els.map((e) => e.getAttribute("data-concept"))),
      ]);
    expect(concepts.sort()).toEqual(CONCEPTS);
  });

  test("the_map_draws_every_block_with_its_tone_kind_and_counts", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    const blocks = page.locator(".block");
    await expect(blocks).toHaveCount(10);
    const tones = await blocks.evaluateAll((els) =>
      els.map((e) => e.getAttribute("data-tone")).sort(),
    );
    expect(new Set(tones)).toEqual(
      new Set(["service", "library", "store", "infra"]),
    );
    const orders = page.locator(".block", { hasText: /^orders\s*service/ });
    await expect(orders).toContainText("14 component · 23 code");
    await expect(page.locator(".block[data-holds] .block-lid")).toHaveCount(5);
    // Lifted edges, with their counts where more than one relation is lifted.
    await expect(page.locator(".react-flow__edge")).toHaveCount(22);
    await expect(page.locator(".react-flow__edge-text")).toHaveText([
      "3",
      "2",
      "2",
      "2",
    ]);
    // The screen-reader outline names every block with its kind and level.
    await expect(page.getByRole("tree")).toBeAttached();
    await expect(
      page.getByRole("treeitem", { name: /^redis, queue, container/ }),
    ).toBeAttached();
  });

  test("a_new_structure_is_laid_out_in_the_worker_and_written_back", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    await expect.poll(() => served.layouts.length).toBe(1);
    const layout = served.layouts[0]!;
    expect(layout.view_key).toBe("container|all|");
    expect(layout.structure_hash).toMatch(/^[0-9a-f]{16}$/);
    expect(Object.keys(layout.positions)).toHaveLength(10);
    expect(
      served.requests.some((r) => r.path === "/assets/elk-worker.min.js"),
    ).toBe(true);

    // The next tab gets the stored positions with the graph and never starts ELK.
    const before = served.requests.length;
    const other = await page
      .context()
      .browser()!
      .newPage({ viewport: DESKTOP });
    await openMap(other, served);
    const later = served.requests.slice(before);
    expect(later.some((r) => r.path === "/assets/elk-worker.min.js")).toBe(
      false,
    );
    expect(served.layouts).toHaveLength(1);
    const box = await other
      .locator(".react-flow__node")
      .first()
      .evaluate((e) => (e as HTMLElement).style.transform);
    expect(box).toMatch(/translate\(/);
    await other.close();
  });

  test("the_login_code_buys_a_token_and_leaves_the_address", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    expect(page.url()).not.toContain("code=");
    expect(
      await page.evaluate(() => sessionStorage.getItem("planroom.token")),
    ).toMatch(/^[0-9a-f]{32}$/);
    const sessions = () =>
      served.requests.filter((r) => r.path === "/api/v1/session").length;
    expect(sessions()).toBe(1);
    // A reload within one serve stays signed in, with no new code.
    await page.reload();
    await page.locator(".map[data-ready]").waitFor();
    expect(sessions()).toBe(1);
    // Every API request carried the exact Origin, except the same-origin stream GET.
    for (const r of served.requests.filter(
      (r) => r.path.startsWith("/api/") && r.method !== "GET",
    )) {
      expect(r.origin).toBe(new URL(served.url).origin);
    }
  });

  test("a_tab_without_a_link_needs_a_fresh_one", async ({ page, served }) => {
    await page.goto(served.url);
    await expect(
      page.getByText("This tab needs a fresh link: press e in the pane"),
    ).toBeVisible();
    await expect(page.locator("[data-concept]")).toHaveCount(0);
    expect(served.requests.filter((r) => r.path.startsWith("/api/"))).toEqual(
      [],
    );
  });

  test("a_spent_code_needs_a_fresh_link", async ({ browser, served }) => {
    const url = served.loginUrl();
    const first = await browser.newPage({ viewport: DESKTOP });
    await first.goto(url);
    await first.locator(".map[data-ready]").waitFor();
    const second = await browser.newPage({ viewport: DESKTOP });
    const errors: string[] = [];
    second.on("console", (m) => m.type() === "error" && errors.push(m.text()));
    await second.goto(url);
    await expect(
      second.getByText("This tab needs a fresh link: press e in the pane"),
    ).toBeVisible();
    // The browser logs the refused exchange itself; nothing else may fail.
    expect(errors.every((e) => e.includes("401"))).toBe(true);
    await first.close();
    await second.close();
  });

  test("the_card_and_the_next_action_take_one_tap", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    const card = page.locator("[data-concept=decisions] .face-up");
    await expect(card.getByRole("heading")).toHaveText(
      "Apply Claude's 2 edits to the worker group?",
    );
    await expect(
      card.getByRole("button", { name: "Accept both" }),
    ).toHaveAttribute("aria-pressed", "true");
    await card.getByRole("button", { name: "Accept both" }).click();
    await expect
      .poll(() => served.runs.map((r) => r.path))
      .toContain("/api/v1/decide");
    expect(served.runs.at(-1)?.body).toEqual({
      argv: ["card_01JB7Q", "accept"],
    });
  });

  test("the_tray_badge_counts_the_diffs_and_opens_the_tray", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    const badge = page.locator("[data-concept=diffs]");
    await expect(badge).toContainText("2");
    await badge.click();
    await expect(page.locator(".tray-row")).toHaveText([
      "worker → platform: + calls orders-api",
      "storefront: ~ checkout moved to cart",
    ]);
  });

  test("a_live_event_refetches_the_map_and_the_pane", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    await expect
      .poll(() => served.requests.some((r) => r.path === "/api/v1/stream"))
      .toBe(true);
    const maps = () =>
      served.requests.filter((r) => r.path === "/api/v1/map").length;
    const before = maps();
    await page.waitForTimeout(350);
    served.push("event", {
      type: "event",
      seq: 413,
      kind: "edit",
      target: null,
      project: null,
      actor: { kind: "person", id: "p", name: null },
      status: "applied",
      summary: "Renamed a block.",
    });
    await expect.poll(maps).toBe(before + 1);
  });

  test("search_dims_what_does_not_match_and_enter_selects_the_first_match", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    await page.getByRole("searchbox", { name: "Search blocks" }).fill("orders");
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(2);
    await page.getByRole("searchbox", { name: "Search blocks" }).press("Enter");
    await expect(page.locator(".block[data-selected]")).toContainText("orders");
  });

  test("reduced_motion_turns_movement_into_a_crossfade", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    await page.getByRole("button", { name: "component" }).click();
    await expect(page.locator(".map")).toHaveAttribute("data-moment", "dive");
    expect(
      await page
        .locator(".map")
        .evaluate((e) => getComputedStyle(e).animationName),
    ).toBe("lid-lift");
    expect(
      await page
        .locator(".button")
        .first()
        .evaluate((e) => getComputedStyle(e).transitionDuration),
    ).not.toBe("0s");

    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.getByRole("button", { name: "container" }).click();
    await expect(page.locator(".map")).toHaveAttribute("data-moment", "rise");
    expect(
      await page
        .locator(".map")
        .evaluate((e) => getComputedStyle(e).animationName),
    ).toBe("crossfade");
    expect(
      await page
        .locator(".button")
        .first()
        .evaluate((e) => getComputedStyle(e).transitionProperty),
    ).toBe("none");
  });
});

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("exactly_five_concepts_on_the_phone", async ({ page, served }) => {
    await openMap(page, served);
    const concepts = await page
      .locator("[data-concept]")
      .evaluateAll((els) => [
        ...new Set(els.map((e) => e.getAttribute("data-concept"))),
      ]);
    expect(concepts.sort()).toEqual(CONCEPTS);
  });

  test("the_top_row_holds_four_targets_at_most_every_target_is_44px_and_nothing_overflows", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    const row = page.locator(".top-row");
    const targets = row.locator("button, input");
    expect(await targets.count()).toBeLessThanOrEqual(4);
    const rowBox = (await row.boundingBox())!;
    expect(rowBox.height).toBeLessThan(70);
    for (const target of await page
      .locator("button:visible, input:visible")
      .all()) {
      const box = (await target.boundingBox())!;
      expect(
        box.width,
        await target.evaluate((e) => e.outerHTML.slice(0, 80)),
      ).toBeGreaterThanOrEqual(44);
      expect(
        box.height,
        await target.evaluate((e) => e.outerHTML.slice(0, 80)),
      ).toBeGreaterThanOrEqual(44);
    }
    const widths = await page.evaluate(() => [
      document.documentElement.scrollWidth,
      document.body.scrollWidth,
    ]);
    expect(Math.max(...widths)).toBeLessThanOrEqual(390);
  });

  test("the_sheet_peeks_with_the_card_and_opens_to_half_with_the_tray", async ({
    page,
    served,
  }) => {
    await openMap(page, served);
    const sheet = page.locator(".sheet");
    await expect(sheet).toHaveAttribute("data-detent", "peek");
    await expect(
      sheet.getByRole("button", { name: "Accept both" }),
    ).toBeVisible();
    await sheet.locator("[data-concept=diffs]").tap();
    await expect(sheet).toHaveAttribute("data-detent", "half");
    await expect(sheet.locator(".tray-row")).toHaveCount(2);
  });

  test("search_opens_inside_the_lid", async ({ page, served }) => {
    await openMap(page, served);
    await page.getByRole("button", { name: "Search blocks" }).tap();
    await page.getByRole("searchbox", { name: "Search blocks" }).fill("kit");
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(1);
  });
});

test.describe("an unscanned workspace", () => {
  for (const viewport of [DESKTOP, PHONE]) {
    test(`the_empty_map_offers_its_one_next_action at ${viewport.width}`, async ({
      page,
    }) => {
      const served = await serve({ map: "map-empty.json" });
      await page.setViewportSize(viewport);
      await page.goto(served.loginUrl());
      const empty = page.locator("[data-empty]");
      await expect(empty.locator("img")).toBeVisible();
      await expect(page.getByRole("button")).toHaveCount(1);
      await page
        .getByRole("button", { name: "Map these 5 repos? About 2 s." })
        .click();
      await expect
        .poll(() => served.runs)
        .toEqual([
          { path: "/api/v1/scan", body: { argv: ["/home/you/shop"] } },
        ]);
      await served.close();
    });
  }
});
