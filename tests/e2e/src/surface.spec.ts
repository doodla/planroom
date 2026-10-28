/**
 * The canvas against the real engine: `planroom serve` on a scanned copy of the fixture, signed in
 * through its login code. The five concepts, the fixture's real blocks, the ELK layout and its
 * stored positions, the login, the next action, a CLI commit reaching the open page over the
 * stream, search, the phone layout, reduced motion, and the empty map of a workspace no scan has
 * touched, whose next action scans it. Every test also fails on a console error. The states the
 * engine can't produce yet are in `standin.spec.ts`.
 */

import type { MapView } from "@planroom/contract";
import type { Locator, Page } from "@playwright/test";

import { startEngine, type Engine } from "./engine";
import {
  CONCEPTS,
  DESKTOP,
  PHONE,
  expect,
  openMap,
  settled,
  test,
} from "./fixtures";

/** The engine's own map of the workspace, as the canvas asks for it. */
/**
 * The engine's whole top-level map, as the canvas reads it over HTTP: the CLI keeps each reply to
 * its budget and pages, so the pages are followed to the end and their blocks joined.
 */
function engineMap(engine: Engine): MapView {
  const args = ["map", "--detail", "graph"];
  const first = engine.cli(args);
  const view = first.result as MapView;
  let next = first.meta.next;
  while (next) {
    const page = engine.cli([...args, "--cursor", next]);
    const more = page.result as MapView;
    view.nodes.push(...more.nodes);
    const seen = new Set(view.edges.map((e) => `${e.kind} ${e.from} ${e.to}`));
    view.edges.push(
      ...more.edges.filter((e) => !seen.has(`${e.kind} ${e.from} ${e.to}`)),
    );
    next = page.meta.next;
  }
  return view;
}

/**
 * How much of `target` a press reaches, in CSS px: `document.elementFromPoint` sampled once a
 * pixel across its middle row, then down the middle of what that row hit. A neighbour that paints
 * over part of a target takes those hits, which its bounding box never shows (ux-b).
 */
async function hitSize(
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
async function topRowHolds(page: Page): Promise<void> {
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
 * The map shows the default lens and the fixture has no project, and until the lens picker lands
 * neither picker has anything to pick: each shows what it holds, isn't a button, and a press opens
 * no sheet.
 */
async function offersOnlyWhatWorks(page: Page, phone: boolean) {
  const lens = page.locator(".top-row [data-concept=lenses]");
  const shelf = page.locator(".top-row [data-concept=intent]");
  await expect(lens.locator(".yarn")).toBeVisible();
  // The lens's name; on the phone it's for a screen reader, beside the swatch.
  await expect(lens).toContainText("runtime");
  await expect(shelf).toContainText("No project");
  await expect(lens.getByRole("button")).toHaveCount(0);
  await expect(shelf.getByRole("button")).toHaveCount(0);
  for (const control of [lens, shelf]) {
    if (phone) {
      await control.tap();
    } else {
      await control.click();
    }
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.locator(".picker-sheet")).toHaveCount(0);
  }
}

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("exactly_five_concepts", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    const concepts = await page
      .locator("[data-concept]")
      .evaluateAll((els) => [
        ...new Set(els.map((e) => e.getAttribute("data-concept"))),
      ]);
    expect(concepts.sort()).toEqual(CONCEPTS);
  });

  test("the_map_draws_the_fixtures_real_blocks_each_with_its_kind_and_counts", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    expect(view.nodes).toHaveLength(10);
    await openMap(page, await engine.loginUrl());
    const blocks = page.locator(".block");
    await expect(blocks).toHaveCount(view.nodes.length);
    expect(
      (await page.locator(".block-name").allTextContents()).sort(),
    ).toEqual(view.nodes.map((n) => n.name).sort());
    // Each block draws in its kind's tone from the code pack (DR-46): a container in its block
    // color, and a repo, whose kind has no tone, as a plain card.
    const looks = await blocks.evaluateAll((els) => {
      const probe = document.createElement("div");
      probe.style.background = "var(--card)";
      document.body.append(probe);
      const card = getComputedStyle(probe).backgroundColor;
      probe.remove();
      return els.map((e) => ({
        name: e.querySelector(".block-name")?.textContent ?? "",
        tone: e.getAttribute("data-tone"),
        card: getComputedStyle(e).backgroundColor === card,
      }));
    });
    const tones = new Map(view.nodes.map((n) => [n.name, n.tone ?? "none"]));
    for (const look of looks) {
      expect(look.tone).toBe(tones.get(look.name));
      expect(look.card).toBe(look.tone === "none");
    }
    // The five containers no repo holds carry a tone; the five repos don't.
    expect(looks.filter((l) => l.tone !== "none")).toHaveLength(5);
    const orders = page.locator(".block", { hasText: /^orders\s*repo/ });
    await expect(orders.locator(".block-kind")).toHaveText("repo");
    // 14 directories, its package, its API spec and the repo's two authors at the component level;
    // its 23 files and the 5 endpoints the link snapshot makes of its routes at the code level.
    await expect(orders.locator(".block-counts")).toHaveText(
      "1 container · 18 component · 28 code",
    );
    // The code pack's image roles give a compose service its kind: redis is a queue and orders-db
    // a datastore.
    await expect(
      page.locator(".block", { hasText: /^redis/ }).locator(".block-kind"),
    ).toHaveText("queue");
    await expect(
      page.locator(".block", { hasText: /^orders-db/ }).locator(".block-kind"),
    ).toHaveText("datastore");
    // The five repos hold others and show a lid, and the link snapshot's relations are lifted
    // between the blocks, one edge each.
    await expect(page.locator(".block[data-holds] .block-lid")).toHaveCount(5);
    expect(view.edges.length).toBeGreaterThan(0);
    await expect(page.locator(".react-flow__edge")).toHaveCount(
      view.edges.length,
    );
    // The screen-reader outline names every block with its kind and level.
    await expect(page.getByRole("tree")).toBeAttached();
    await expect(
      page.getByRole("treeitem", { name: /^redis, queue, container/ }),
    ).toBeAttached();
  });

  test("a_fresh_structure_is_laid_out_in_the_worker_and_its_stored_positions_survive_a_reload", async ({
    page,
    engine,
    sent,
  }) => {
    const key = engineMap(engine).layout!;
    const put = page.waitForRequest(
      (r) => r.method() === "PUT" && r.url().endsWith("/api/v1/layout"),
    );
    await openMap(page, await engine.loginUrl());
    const body = (await put).postDataJSON() as {
      view_key: string;
      structure_hash: string;
      positions: Record<string, { x: number; y: number }>;
    };
    expect(body.view_key).toBe(key.view_key);
    expect(body.structure_hash).toBe(key.structure_hash);
    expect(Object.keys(body.positions)).toHaveLength(10);
    expect(sent.some((r) => r.path === "/assets/elk-worker.min.js")).toBe(true);
    // The engine serves the stored positions with the graph from now on.
    await expect
      .poll(() => engineMap(engine).nodes.every((n) => n.position !== null))
      .toBe(true);
    const where = () =>
      page
        .locator(".react-flow__node")
        .evaluateAll((els) =>
          els.map((e): [string, string] => [
            e.getAttribute("data-id") ?? "",
            (e as HTMLElement).style.transform,
          ]),
        );
    const before = await where();

    // A reload gets the stored positions with the graph and never starts ELK.
    const at = sent.length;
    await page.reload();
    await page.locator(".map[data-ready]").waitFor();
    const later = sent.slice(at);
    expect(later.some((r) => r.path === "/assets/elk-worker.min.js")).toBe(
      false,
    );
    expect(later.some((r) => r.path === "/api/v1/layout")).toBe(false);
    expect(await where()).toEqual(before);
    expect(before).toHaveLength(10);
    expect(before.every(([, t]) => /translate\(/.test(t))).toBe(true);
  });

  test("the_login_code_buys_a_token_and_leaves_the_address", async ({
    page,
    engine,
    sent,
  }) => {
    const stream = page.waitForResponse((r) =>
      r.url().includes("/api/v1/stream"),
    );
    await openMap(page, await engine.loginUrl());
    expect(page.url()).not.toContain("code=");
    expect(
      await page.evaluate(() => sessionStorage.getItem("planroom.token")),
    ).toMatch(/^[0-9a-f]{64}$/);
    // The stream is a same-origin GET with no Origin, and the real server takes it.
    expect((await stream).status()).toBe(200);
    const sessions = () =>
      sent.filter((r) => r.path === "/api/v1/session").length;
    expect(sessions()).toBe(1);
    // A reload within one serve stays signed in, with no new code. The first load's requests end
    // first, so the reload aborts none whose Origin the check below reads.
    await expect.poll(() => settled(sent)).toBe(true);
    await page.reload();
    await page.locator(".map[data-ready]").waitFor();
    expect(sessions()).toBe(1);
    // Every API request but the stream's GET carried the exact Origin.
    const origin = new URL(engine.url).origin;
    await expect
      .poll(() =>
        sent
          .filter((r) => r.path.startsWith("/api/") && r.method !== "GET")
          .every((r) => r.origin === origin),
      )
      .toBe(true);
    expect(
      sent.filter((r) => r.path === "/api/v1/stream").map((r) => r.origin),
    ).not.toContain(origin);
  });

  test("a_tab_without_a_link_needs_a_fresh_one", async ({
    page,
    engine,
    sent,
  }) => {
    await page.goto(engine.url);
    await expect(
      page.getByText("This tab needs a fresh link: press e in the pane"),
    ).toBeVisible();
    await expect(page.locator("[data-concept]")).toHaveCount(0);
    expect(sent.filter((r) => r.path.startsWith("/api/"))).toEqual([]);
  });

  test("a_spent_code_needs_a_fresh_link", async ({ browser, engine }) => {
    const url = await engine.loginUrl();
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

  test("with_no_open_decision_the_next_action_is_the_face_up_card_and_takes_one_tap", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const card = page.locator("[data-concept=decisions] .face-up");
    const next = card.getByRole("button", { name: "Open the containers" });
    await expect(next).toBeVisible();
    // The handle runs as its command's request on the real route.
    const ran = page.waitForResponse(
      (r) =>
        r.url().endsWith("/api/v1/map") &&
        (r.request().postDataJSON() as { level?: string }).level ===
          "container",
    );
    await next.click();
    expect((await ran).status()).toBe(200);
  });

  test("the_breadcrumb_names_the_level_and_each_ancestor_goes_back_up", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const crumbs = page.locator(".breadcrumb");
    await expect(crumbs.locator(".lid-name")).toHaveText("shop");
    await page
      .locator(".rail")
      .getByRole("button", { name: "component", exact: true })
      .click();
    await expect(crumbs.locator(".lid-name")).toHaveText("component");
    await crumbs
      .getByRole("button", { name: "container", exact: true })
      .click();
    await expect(crumbs.locator(".lid-name")).toHaveText("container");
    await crumbs.getByRole("button", { name: "shop", exact: true }).click();
    await expect(crumbs.locator(".lid-name")).toHaveText("shop");
    await expect(page.locator(".map[data-ready]")).toBeVisible();
  });

  test("once_its_level_shows_the_next_action_turns_into_the_way_back", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const card = page.locator("[data-concept=decisions] .face-up");
    await card.getByRole("button", { name: "Open the containers" }).click();
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("container");
    await expect(
      card.getByRole("button", { name: "Open the containers" }),
    ).toHaveCount(0);
    await card.getByRole("button", { name: "Back to shop" }).click();
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("shop");
    await expect(
      card.getByRole("button", { name: "Open the containers" }),
    ).toBeVisible();
  });

  test("a_level_with_no_blocks_draws_an_empty_state_with_a_way_back", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await page
      .locator(".rail")
      .getByRole("button", { name: "intent", exact: true })
      .click();
    const empty = page.locator(".level-empty [data-empty]");
    await expect(empty.locator("img")).toBeVisible();
    await expect(empty.getByRole("button")).toHaveCount(1);
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("intent");
    await page
      .locator(".breadcrumb")
      .getByRole("button", { name: "shop", exact: true })
      .click();
    await expect(page.locator(".map[data-ready]")).toBeVisible();
  });

  test("the_lens_switcher_and_the_project_shelf_offer_only_what_works", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await offersOnlyWhatWorks(page, false);
  });

  test("an_empty_tray_shows_its_art_and_the_next_action", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const badge = page.locator("[data-concept=diffs]");
    await expect(badge.locator(".badge")).toHaveText("0");
    await badge.click();
    const drawer = page.locator(".drawer");
    await expect(drawer.locator("[data-empty] img")).toBeVisible();
    await expect(
      drawer.getByRole("button", { name: "Open the containers" }),
    ).toBeVisible();
  });

  test("a_cli_commit_reaches_the_open_page_over_the_stream_within_1s", async ({
    page,
    engine,
  }) => {
    const stream = page.waitForResponse((r) =>
      r.url().includes("/api/v1/stream"),
    );
    await openMap(page, await engine.loginUrl());
    expect((await stream).status()).toBe(200);
    const counts = page
      .locator(".block", { hasText: /^orders\s*repo/ })
      .locator(".block-counts");
    const before = await counts.textContent();
    // Let the first refetch window pass, so the frame refetches at once.
    await page.waitForTimeout(400);

    engine.commit(
      "orders",
      {
        "internal/audit/audit.go":
          "package audit\n\n// Record notes an order event.\nfunc Record(id string) string { return id }\n",
      },
      "Add the audit package",
    );
    const scan = engine.cli(["scan"]);
    expect(scan.ok).toBe(true);
    const committed = Date.now();
    await expect(counts).not.toHaveText(before ?? "", { timeout: 3000 });
    const took = Date.now() - committed;
    const line = `CLI scan to the open page's map (ms): ${took}; ${before} -> ${await counts.textContent()}`;
    console.log(line);
    test.info().annotations.push({ type: "commit", description: line });
    expect(took).toBeLessThan(1000);
  });

  test("search_dims_what_does_not_match_and_enter_selects_the_first_match", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await page.getByRole("searchbox", { name: "Search blocks" }).fill("orders");
    // orders and orders-db.
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(2);
    await page.getByRole("searchbox", { name: "Search blocks" }).press("Enter");
    await expect(page.locator(".block[data-selected]")).toContainText("orders");
  });

  test("reduced_motion_turns_movement_into_a_crossfade", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const rail = page.locator(".rail");
    await rail.getByRole("button", { name: "component", exact: true }).click();
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
    await rail.getByRole("button", { name: "container", exact: true }).click();
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

  test("exactly_five_concepts_on_the_phone", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    const concepts = await page
      .locator("[data-concept]")
      .evaluateAll((els) => [
        ...new Set(els.map((e) => e.getAttribute("data-concept"))),
      ]);
    expect(concepts.sort()).toEqual(CONCEPTS);
  });

  test("the_top_row_holds_four_targets_at_most_every_target_is_44px_and_nothing_overflows", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await topRowHolds(page);
    // Below the top the `⋯` lid joins the row, and the current lid overlaps it (ux-b).
    await page
      .locator(".sheet")
      .getByRole("button", { name: "Open the containers" })
      .first()
      .tap();
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("container");
    await expect(
      page.getByRole("button", { name: "Levels above" }),
    ).toBeVisible();
    await topRowHolds(page);
  });

  test("the_lens_switcher_and_the_project_shelf_offer_only_what_works_on_the_phone", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await offersOnlyWhatWorks(page, true);
  });

  test("the_sheet_peeks_with_the_next_action_and_opens_to_half_with_the_tray", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const sheet = page.locator(".sheet");
    await expect(sheet).toHaveAttribute("data-detent", "peek");
    await expect(
      sheet.getByRole("button", { name: "Open the containers" }).first(),
    ).toBeVisible();
    await sheet.locator("[data-concept=diffs]").tap();
    await expect(sheet).toHaveAttribute("data-detent", "half");
    await expect(sheet.locator("[data-empty] img")).toBeVisible();
  });

  test("the_phone_always_goes_back_up_through_the_ancestors_lid", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const sheet = page.locator(".sheet");
    const lid = page.locator(".breadcrumb .lid-name");
    await expect(lid).toHaveText("shop");
    // No lid above the top.
    await expect(
      page.getByRole("button", { name: "Levels above" }),
    ).toHaveCount(0);
    await sheet.getByRole("button", { name: "Open the containers" }).tap();
    await expect(lid).toHaveText("container");
    // The sheet stops offering what it just did, and the row keeps four targets.
    await expect(
      sheet.getByRole("button", { name: "Open the containers" }),
    ).toHaveCount(0);
    expect(
      await page.locator(".top-row").locator("button, input").count(),
    ).toBeLessThanOrEqual(4);
    await page.getByRole("button", { name: "Levels above" }).tap();
    await page
      .getByRole("dialog", { name: "Levels above" })
      .getByRole("button", { name: "shop" })
      .tap();
    await expect(lid).toHaveText("shop");
    await expect(
      sheet.getByRole("button", { name: "Open the containers" }).first(),
    ).toBeVisible();
  });

  test("search_opens_inside_the_lid", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    await page.getByRole("button", { name: "Search blocks" }).tap();
    await page.getByRole("searchbox", { name: "Search blocks" }).fill("kit");
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(1);
  });
});

// The real `serve` starts in a workspace no scan has touched, so the empty map comes from it.
test.describe("an unscanned workspace", () => {
  for (const viewport of [DESKTOP, PHONE]) {
    test(`the_empty_map_offers_its_one_next_action at ${viewport.width}`, async ({
      page,
    }) => {
      const engine = await startEngine({ scanned: false });
      try {
        await page.setViewportSize(viewport);
        await page.goto(await engine.loginUrl());
        const empty = page.locator("[data-empty]");
        await expect(empty.locator("img")).toBeVisible();
        // The engine's line says why the page is empty, beside its one short action.
        await expect(empty.locator(".empty-line")).toHaveText(
          "Nothing is mapped here yet; 5 repos can be.",
        );
        await expect(page.getByRole("button")).toHaveCount(1);
        await page
          .getByRole("button", { name: "Map these 5 repos? About 2 s." })
          .click();
        // The handle runs as scan's request on the real route, and the map that scan made
        // replaces the empty state.
        await page.locator(".map[data-ready]").waitFor();
        const view = engineMap(engine);
        expect(view.nodes).toHaveLength(10);
        await expect(page.locator(".block")).toHaveCount(view.nodes.length);
      } finally {
        engine.close();
      }
    });
  }
});
