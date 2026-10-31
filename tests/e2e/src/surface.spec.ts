/**
 * The canvas against the real engine: `planroom serve` on a scanned copy of the fixture, signed in
 * through its login code. The five concepts, the fixture's real blocks, the ELK layout and its
 * stored positions, the login, the next action, a CLI commit reaching the open page over the
 * stream, search, the phone layout, reduced motion, and the empty map of a workspace no scan has
 * touched, whose next action scans it. Every test also fails on a console error. The states the
 * engine can't produce yet are in `standin.spec.ts`.
 */

import { CANVAS_STRINGS, type MapNode, type MapView } from "@planroom/contract";
import type { Browser, Locator, Page } from "@playwright/test";

import { hostArtifact, openArtifact, shareFixture } from "./artifact-host";
import {
  breakRepo,
  initRepo,
  removeRepos,
  startEngine,
  type Engine,
} from "./engine";
import {
  CONCEPTS,
  DESKTOP,
  PHONE,
  conceptsOf,
  doublePress,
  drawnIds,
  expect,
  goToLevel,
  lensItems,
  openMap,
  pressEdge,
  tokenColor,
  settled,
  test,
  topRowHolds,
} from "./fixtures";

/** The engine's own map of the workspace, as the canvas asks for it. */
/**
 * The engine's whole top-level map, as the canvas reads it over HTTP: the CLI keeps each reply to
 * its budget and pages, so the pages are followed to the end and their blocks joined.
 */
function engineMap(engine: Engine, more: readonly string[] = []): MapView {
  const args = ["map", "--detail", "graph", ...more];
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

/** A block's counts in the ladder's words: its level's `one` for 1, else `many`, top level first. */
function countWords(view: MapView, node: MapNode): string[] {
  return (view.levels ?? []).flatMap((level) => {
    const count = node.counts[level.id] ?? 0;
    return count > 0
      ? [
          (count === 1 ? level.one : level.many).replace(
            "{count}",
            String(count),
          ),
        ]
      : [];
  });
}

/**
 * The lens switcher lists the lenses `ask` lists, each pick changes only the lens (the shown level
 * stays) and the swatch takes that lens's yarn; the shelf has no project to pick, so it stays flat.
 * On the desktop the open sheet leaves the face-up card pressable.
 */
async function offersOnlyWhatWorks(page: Page, engine: Engine, phone: boolean) {
  const lens = page.locator(".top-row [data-concept=lenses]");
  const shelf = page.locator(".top-row [data-concept=intent]");
  const press = (control: Locator) => (phone ? control.tap() : control.click());
  const open = () =>
    press(lens.getByRole("button", { name: CANVAS_STRINGS.lenses }));
  const items = lensItems(engine);
  expect(items.length, "the listing's lenses").toBeGreaterThan(1);
  await expect(lens.locator(".yarn")).toBeVisible();
  await expect(shelf).toContainText(CANVAS_STRINGS.no_project);
  await expect(shelf.getByRole("button")).toHaveCount(0);

  await open();
  const sheet = page.locator(".picker-sheet");
  await expect(sheet).toBeVisible();
  const theme = sheet.getByRole("radiogroup", { name: CANVAS_STRINGS.theme });
  await expect(theme).toBeVisible();
  await expect(theme.getByRole("radio")).toHaveCount(3);
  await expect(sheet.locator(".picker-entry")).toHaveText(
    items.map((i) => i.name),
  );
  if (!phone) {
    // The sheet ends before the side column: the face-up card under it stays pressable.
    const card = page.locator("[data-concept=decisions] .face-up");
    const button = card.getByRole("button").first();
    const sheetBox = (await sheet.boundingBox())!;
    const cardBox = (await card.boundingBox())!;
    const overlaps =
      sheetBox.x < cardBox.x + cardBox.width &&
      cardBox.x < sheetBox.x + sheetBox.width &&
      sheetBox.y < cardBox.y + cardBox.height &&
      cardBox.y < sheetBox.y + sheetBox.height;
    expect(overlaps, "the sheet and the face-up card").toBe(false);
    const centre = (await button.boundingBox())!;
    expect(
      await page.evaluate(
        ([x, y]) => {
          const hit = document.elementFromPoint(x!, y!);
          return hit?.closest("button")?.textContent ?? null;
        },
        [centre.x + centre.width / 2, centre.y + centre.height / 2],
      ),
    ).toBe(await button.textContent());
  }
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);

  const lid = page.locator(".breadcrumb .lid-name");
  const shown = await lid.textContent();
  for (const item of items) {
    await open();
    await press(
      page
        .locator(".picker-sheet")
        .getByRole("button", { name: item.name, exact: true }),
    );
    const view = engine.cli(["map", "--detail", "graph", "--lens", item.id])
      .result as MapView;
    const yarn = await tokenColor(page, `yarn-${view.yarn!}`);
    await expect(
      lens.locator(".picker-label, .visually-hidden").last(),
    ).toHaveText(item.name);
    await expect
      .poll(() =>
        lens.locator(".yarn").evaluate((e) => getComputedStyle(e).color),
      )
      .toBe(yarn);
    await expect(lid).toHaveText(shown!);
    await expect(page.locator(".map[data-ready]")).toBeVisible();
  }
  await expect(shelf.getByRole("button")).toHaveCount(0);
}

/** A new page of its own at the size, with every console error and page error it logs. */
async function ownPage(browser: Browser, phone: boolean) {
  const page = await browser.newPage(
    phone
      ? { viewport: PHONE, hasTouch: true, isMobile: true }
      : { viewport: DESKTOP },
  );
  const errors: string[] = [];
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  page.on("pageerror", (e) => errors.push(e.message));
  return { page, errors };
}

/** The second step of the chain: the first level a press loads, since the first is the top. */
function secondStep(engine: Engine) {
  return (engineMap(engine).levels ?? []).filter((l) => !l.apart)[1]!;
}

/**
 * The page with its engine gone: every request to the API fails after the map has drawn, and a
 * press that loads takes the page. `check` looks at that page; then the requests come back, Retry
 * is pressed and the map draws. The browser logs each aborted request itself and nothing else may
 * fail.
 */
async function whileUnreachable(
  browser: Browser,
  engine: Engine,
  phone: boolean,
  check: (page: Page) => Promise<void>,
): Promise<void> {
  const { page, errors } = await ownPage(browser, phone);
  try {
    await openMap(page, await engine.loginUrl());
    await page.route("**/api/v1/**", (route) => route.abort());
    await goToLevel(page, secondStep(engine).label, phone);
    await expect(page.locator(".empty-line")).toHaveText(
      CANVAS_STRINGS.unreachable,
    );
    await check(page);
    await page.unroute("**/api/v1/**");
    const retry = page.getByRole("button", { name: CANVAS_STRINGS.retry });
    if (phone) {
      await retry.tap();
    } else {
      await retry.click();
    }
    await page.locator(".map[data-ready]").waitFor();
    expect(
      errors.filter((e) => !e.includes("net::ERR_FAILED")),
      "console and page errors other than the aborted requests",
    ).toEqual([]);
  } finally {
    await page.close();
  }
}

/**
 * Every control the canvas draws sits inside one of the five concepts (R14, DR-77), on the main
 * view and on each page and sheet it can show. Pairing and Ask Claude draw no control in M1: their
 * places are the card stack and the evidence sheet, and signing in is a page with no control.
 */
async function everyControlHasAConcept(
  browser: Browser,
  page: Page,
  engine: Engine,
  phone: boolean,
): Promise<void> {
  const press = (target: Locator) => (phone ? target.tap() : target.click());
  await openMap(page, await engine.loginUrl());
  expect(await conceptsOf(page)).toEqual(CONCEPTS);

  // The sheet that holds the levels on the phone and the lenses on the desktop.
  const sheet = phone ? CANVAS_STRINGS.levels_sheet : CANVAS_STRINGS.lenses;
  await press(page.getByRole("button", { name: sheet }));
  await expect(page.getByRole("dialog", { name: sheet })).toBeVisible();
  await conceptsOf(page);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: sheet })).toHaveCount(0);

  // The evidence sheet, which opens the phone's bottom sheet to half.
  await pressEdge(page, page.locator(".react-flow__edge").first(), phone);
  const evidence = page.getByRole("dialog", { name: CANVAS_STRINGS.evidence });
  await expect(evidence).toBeVisible();
  await conceptsOf(page);
  await page.keyboard.press("Escape");
  await expect(evidence).toHaveCount(0);

  if (phone) {
    await press(page.getByRole("button", { name: CANVAS_STRINGS.sheet_half }));
    await expect(page.locator(".sheet")).toHaveAttribute("data-detent", "half");
    await conceptsOf(page);
    await press(page.getByRole("button", { name: CANVAS_STRINGS.sheet_peek }));
  }

  // A dive.
  const top = engineMap(engine);
  const below = secondStep(engine).id;
  const block = top.nodes.find((n) => (n.counts[below] ?? 0) > 0)!;
  await doublePress(
    page,
    page.locator(`.react-flow__node[data-id="${block.id}"]`),
    phone,
  );
  await expect(page.locator(".breadcrumb .lid-name")).toHaveText(block.name);
  await expect(page.locator(".map[data-ready]")).toBeVisible();
  await conceptsOf(page);

  // The page of a workspace no scan has touched.
  const unscanned = await startEngine({ scanned: false });
  const bare = await ownPage(browser, phone);
  try {
    await bare.page.goto(await unscanned.loginUrl());
    await bare.page.locator("[data-empty] img").waitFor();
    expect(await conceptsOf(bare.page)).toContain("levels");
    expect(bare.errors).toEqual([]);
  } finally {
    await bare.page.close();
    unscanned.close();
  }

  // The page of an engine that stopped answering.
  await whileUnreachable(browser, engine, phone, async (gone) => {
    expect(await conceptsOf(gone)).toContain("levels");
  });

  // Signing in is a page with no control and no concept.
  const login = await ownPage(browser, phone);
  try {
    await login.page.goto(engine.url);
    await expect(
      login.page.getByText(CANVAS_STRINGS.needs_login),
    ).toBeVisible();
    await expect(login.page.locator("button, input")).toHaveCount(0);
    await expect(login.page.locator("[data-concept]")).toHaveCount(0);
  } finally {
    await login.page.close();
  }

  // The Artifact page.
  const ws = shareFixture();
  const host = await hostArtifact(ws.share);
  const shared = await ownPage(browser, phone);
  try {
    await openArtifact(shared.page, ws, host, true);
    expect(await conceptsOf(shared.page)).toEqual(CONCEPTS);
    expect(shared.errors).toEqual([]);
  } finally {
    await shared.page.close();
    await host.close();
    ws.close();
  }
}

/**
 * No state is blank (design-language.md, "Empty states"): the art, the engine's words (`line`,
 * when the engine gives one for the state, else whatever the state says), at most one primary
 * button, and never a map with no blocks. The words are the empty line or, when the one action
 * carries them, that button's label.
 */
async function emptyStateHolds(
  page: Page,
  scope: Locator,
  line?: string,
): Promise<void> {
  await expect(scope.locator("img.empty-art")).toBeVisible();
  const primary = scope.locator("button.primary");
  expect(await primary.count()).toBeLessThanOrEqual(1);
  const text = scope.locator(".empty-line");
  const words = (await text.count()) > 0 ? text.first() : primary.first();
  await expect(words).toHaveText(/\S/);
  if (line !== undefined) {
    await expect(words).toHaveText(line);
  }
  if ((await page.locator(".map").count()) > 0) {
    expect(
      await page.locator(".map .react-flow__node").count(),
    ).toBeGreaterThan(0);
  }
}

/** The empty state the engine reports for `map` with `more`, such as `--level L`. */
function engineEmpty(engine: Engine, more: readonly string[] = []) {
  return (engine.cli(["map", ...more]).result as MapView).empty ?? null;
}

/** Every state with no blocks, the local ones and the Artifact's, at one size. */
async function everyEmptyStateHolds(
  browser: Browser,
  phone: boolean,
): Promise<void> {
  const press = (target: Locator) => (phone ? target.tap() : target.click());
  const fresh = async (
    options: Parameters<typeof startEngine>[0],
    then: (engine: Engine, page: Page) => Promise<void>,
    before?: (engine: Engine) => void,
  ) => {
    const engine = await startEngine(options);
    before?.(engine);
    const own = await ownPage(browser, phone);
    try {
      await own.page.goto(await engine.loginUrl());
      await then(engine, own.page);
      expect(own.errors).toEqual([]);
    } finally {
      await own.page.close();
      engine.close();
    }
  };
  const emptyOf = (page: Page) => page.locator("[data-empty]");
  // The line a next action the canvas can't run is drawn as.
  const inPaneLine = (label: string) =>
    CANVAS_STRINGS.next_in_pane.replace("{label}", label);
  const startPlans =
    (...titles: string[]) =>
    (engine: Engine) => {
      for (const title of titles) {
        engine.cli(["intent", "new", title], "agent:aaaaaaaa:chat");
      }
    };

  // A workspace no scan has touched, one with no repo, one with a plan and no repo, and one whose
  // repos have no commit.
  const states: [
    string,
    Parameters<typeof startEngine>[0],
    ((engine: Engine) => void)?,
  ][] = [
    ["not_scanned", { scanned: false }],
    ["no_repos", { scanned: false, prepare: removeRepos }],
    [
      "plan_only",
      { scanned: false, prepare: removeRepos },
      startPlans("Add a billing service"),
    ],
    [
      "repos_unborn",
      {
        scanned: false,
        prepare: (root) => {
          removeRepos(root);
          initRepo(root, "alpha");
          initRepo(root, "beta");
        },
      },
    ],
  ];
  for (const [reason, options, before] of states) {
    await fresh(
      options,
      async (engine, page) => {
        const empty = engineEmpty(engine)!;
        expect(empty.reason, "the engine's reason").toBe(reason);
        await emptyOf(page).locator("img").waitFor();
        await emptyStateHolds(page, emptyOf(page), empty.line!);
        if (reason === "no_repos" || reason === "plan_only") {
          // The canvas can't run this action, so it names the pane that can instead of a button.
          await expect(emptyOf(page).locator("button")).toHaveCount(0);
          await expect(emptyOf(page).locator(".empty-elsewhere")).toHaveText(
            inPaneLine(empty.next.label),
          );
        }
      },
      before,
    );
  }

  // Picking a project that holds no items yet lands on its empty view, whose next action is
  // another command the canvas can't run: in the scanned fixture, and in a plan-only workspace,
  // whose top keeps the shell so its project shelf is on screen.
  const pickings: [boolean, Parameters<typeof startEngine>[0]][] = [
    [true, {}],
    [false, { scanned: false, prepare: removeRepos }],
  ];
  for (const [scanned, options] of pickings) {
    await fresh(
      options,
      async (engine, page) => {
        if (scanned) {
          await page.locator(".map[data-ready]").waitFor();
        }
        const shelf = page.locator(".top-row [data-concept=intent]");
        await press(
          shelf.getByRole("button", { name: CANVAS_STRINGS.projects }),
        );
        await press(
          page
            .locator(".picker-sheet .picker-entry")
            .filter({ hasText: "Add a billing service" }),
        );
        await emptyOf(page).locator("img").waitFor();
        const listed = engine.cli(["intent", "list"]).result as {
          projects: { id: string; name: string }[];
        };
        const project = listed.projects.find(
          (p) => p.name === "Add a billing service",
        )!;
        const empty = engineEmpty(engine, [
          "--level",
          "intent",
          "--focus",
          project.id,
        ])!;
        expect(empty.reason, "the engine's reason").toBe("no_plan_items");
        await emptyStateHolds(page, emptyOf(page), empty.line!);
        await expect(emptyOf(page).locator("button")).toHaveCount(0);
        await expect(emptyOf(page).locator(".empty-elsewhere")).toHaveText(
          inPaneLine(empty.next.label),
        );
      },
      startPlans("Add a billing service", "Add a search page"),
    );
  }

  // The scanned fixture: the level apart from the chain, every lens at every level, the tray.
  await fresh({}, async (engine, page) => {
    await page.locator(".map[data-ready]").waitFor();
    const levels = engineMap(engine).levels ?? [];
    const apart = levels.find((l) => l.apart)!;
    const intent = engineEmpty(engine, ["--level", apart.id])!;
    expect(intent.reason).toBe("no_intent");
    await goToLevel(page, apart.label, phone);
    await emptyOf(page).locator("img").waitFor();
    await emptyStateHolds(page, emptyOf(page), intent.line!);
    await goToLevel(page, levels.find((l) => !l.apart)!.label, phone);
    await page.locator(".map[data-ready]").waitFor();

    // A lens that shows nothing at a level says so there. No loaded lens declares kinds, so the
    // engine reports none today; the loop covers the state once one does.
    const nothing: { lens: string; name: string; level: string }[] = [];
    for (const item of lensItems(engine)) {
      for (const level of levels) {
        const shown = engineEmpty(engine, [
          "--level",
          level.id,
          "--lens",
          item.id,
        ]);
        if (shown?.reason === "lens_shows_nothing") {
          nothing.push({ lens: item.id, name: item.name, level: level.label });
        }
      }
    }
    expect(
      nothing,
      "lens and level pairs the engine says show nothing",
    ).toHaveLength(0);
    for (const one of nothing) {
      await press(page.getByRole("button", { name: CANVAS_STRINGS.lenses }));
      await press(
        page
          .locator(".picker-sheet")
          .getByRole("button", { name: one.name, exact: true }),
      );
      await goToLevel(page, one.level, phone);
      await emptyOf(page).locator("img").waitFor();
      await emptyStateHolds(page, emptyOf(page));
    }

    // The empty diff tray, with the pane's next action.
    if (phone) {
      await press(
        page.getByRole("button", { name: CANVAS_STRINGS.sheet_half }),
      );
    } else {
      await press(page.locator("[data-concept=diffs]"));
    }
    await emptyStateHolds(
      page,
      page.locator(phone ? ".sheet-body [data-empty]" : ".drawer [data-empty]"),
    );
  });

  // A tab without a link.
  const login = await ownPage(browser, phone);
  const engine = await startEngine();
  try {
    await login.page.goto(engine.url);
    await emptyStateHolds(
      login.page,
      emptyOf(login.page),
      CANVAS_STRINGS.needs_login,
    );
    await expect(login.page.locator("button")).toHaveCount(0);
  } finally {
    await login.page.close();
  }

  // An engine that stopped answering: the line, and Retry.
  await whileUnreachable(browser, engine, phone, async (gone) => {
    await emptyStateHolds(
      gone,
      gone.locator("[data-empty]"),
      CANVAS_STRINGS.unreachable,
    );
    await expect(
      gone.getByRole("button", { name: CANVAS_STRINGS.retry }),
    ).toBeVisible();
  });
  engine.close();

  // The Artifact: the level apart from the chain is a request state, and a double press on a
  // block with something below only selects.
  const ws = shareFixture();
  const host = await hostArtifact(ws.share);
  const shared = await ownPage(browser, phone);
  try {
    await openArtifact(shared.page, ws, host, true);
    const top = ws.cli(["map", "--detail", "graph"]).result as MapView;
    const apart = (top.levels ?? []).find((l) => l.apart)!;
    await goToLevel(shared.page, apart.label, phone);
    const request = shared.page.locator(".request-state");
    await expect(request).toHaveAttribute("data-request", "unavailable");
    await emptyStateHolds(shared.page, request);
    await press(
      shared.page.getByRole("button", { name: CANVAS_STRINGS.back_to_map }),
    );
    await shared.page.locator(".map[data-ready]").waitFor();

    const below = (top.levels ?? []).filter((l) => !l.apart)[1]!.id;
    const block = top.nodes.find((n) => (n.counts[below] ?? 0) > 0)!;
    const node = shared.page.locator(
      `.react-flow__node[data-id="${block.id}"]`,
    );
    await doublePress(shared.page, node, phone);
    await expect(node.locator(".block")).toHaveAttribute(
      "data-selected",
      "true",
    );
    await shared.page.waitForTimeout(500);
    await expect(request).toHaveCount(0);
    expect(
      await shared.page.evaluate(() =>
        window.__artifactDb!.session.list("inbox/u_test/events"),
      ),
      "the rows the double press wrote",
    ).toEqual([]);
    expect(shared.errors).toEqual([]);
  } finally {
    await shared.page.close();
    await host.close();
    ws.close();
  }
}

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("exactly_five_concepts", async ({ browser, page, engine }) => {
    test.setTimeout(180_000);
    await everyControlHasAConcept(browser, page, engine, false);
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
    // The repo's counts, in the ladder's words from the same reply.
    const repo = view.nodes.find(
      (n) => n.name === "orders" && n.level === view.level,
    )!;
    await expect(orders.locator(".block-count")).toHaveText(
      countWords(view, repo),
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
    await expect(page.getByText(CANVAS_STRINGS.needs_login)).toBeVisible();
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
    await expect(second.getByText(CANVAS_STRINGS.needs_login)).toBeVisible();
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
    await card
      .getByRole("button", {
        name: CANVAS_STRINGS.back_to.replace("{place}", "shop"),
      })
      .click();
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
    await offersOnlyWhatWorks(page, engine, false);
  });

  test("the_rails_first_step_is_the_top_and_the_apart_level_sits_apart", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    const rail = page.locator(".rail");
    const chain = rail.locator("li:not([data-apart]) .rail-step");
    const apart = rail.locator("li[data-apart]");
    const mapRequest = (press: () => Promise<void>) => {
      const sent = page.waitForRequest(
        (r) => r.method() === "POST" && r.url().endsWith("/api/v1/map"),
      );
      return press().then(async () => (await sent).postDataJSON() as object);
    };
    const blocks = async () =>
      (
        await page
          .locator(".react-flow__node")
          .evaluateAll((els) => els.map((e) => e.getAttribute("data-id")))
      ).sort();
    const finest = chain.last();

    await finest.click();
    await expect(page.locator(".breadcrumb .lid-name")).not.toHaveText("shop");
    await page.locator(".map[data-ready]").waitFor();
    const lid = await mapRequest(() =>
      page
        .locator(".breadcrumb")
        .getByRole("button", { name: "shop", exact: true })
        .click(),
    );
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("shop");
    await page.locator(".map[data-ready]").waitFor();
    const fromLid = await blocks();

    await finest.click();
    await expect(page.locator(".breadcrumb .lid-name")).not.toHaveText("shop");
    const step = await mapRequest(() => chain.first().click());
    await page.locator(".map[data-ready]").waitFor();
    await expect(chain.first()).toHaveAttribute("aria-current", "step");
    await expect.poll(blocks).toEqual(fromLid);
    expect(step).toEqual(lid);
    expect(step).not.toHaveProperty("level");
    expect(step).not.toHaveProperty("lens");

    // The apart level is a step of its own, outside the span of the chain's steps.
    await expect(apart).toHaveCount(1);
    const first = (await chain.first().boundingBox())!;
    const last = (await finest.boundingBox())!;
    const box = (await apart.boundingBox())!;
    const inside =
      box.y >= first.y && box.y + box.height <= last.y + last.height;
    expect(inside, "the apart step lies within the chain's span").toBe(false);
    // Every label shows whole at this width.
    for (const label of await rail.locator(".rail-label").all()) {
      const [scroll, client] = await label.evaluate((e) => [
        e.scrollWidth,
        e.clientWidth,
      ]);
      expect(scroll, `${await label.textContent()}`).toBeLessThanOrEqual(
        client,
      );
    }
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

  test("a_commit_before_the_stream_opens_still_reaches_the_page", async ({
    page,
    engine,
  }) => {
    // Hold the stream's request until the commit is in, so the page has read its views before the
    // stream opens.
    const streams: string[] = [];
    let release: () => void = () => undefined;
    const released = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route("**/api/v1/stream**", async (route) => {
      streams.push(route.request().url());
      await released;
      await route.continue();
    });
    await openMap(page, await engine.loginUrl());
    const counts = page
      .locator(".block", { hasText: /^orders\s*repo/ })
      .locator(".block-counts");
    const before = await counts.textContent();

    engine.commit(
      "orders",
      {
        "internal/ledger/ledger.go":
          "package ledger\n\n// Post notes an order entry.\nfunc Post(id string) string { return id }\n",
      },
      "Add the ledger package",
    );
    expect(engine.cli(["scan"]).ok).toBe(true);
    release();
    await expect(counts).not.toHaveText(before ?? "", { timeout: 3000 });
    expect(streams.some((u) => u.includes("since="))).toBe(true);
  });

  test("search_dims_what_does_not_match_and_enter_selects_the_first_match", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await page
      .getByRole("searchbox", { name: CANVAS_STRINGS.search })
      .fill("orders");
    // orders and orders-db.
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(2);
    await page
      .getByRole("searchbox", { name: CANVAS_STRINGS.search })
      .press("Enter");
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

  test("a_dive_into_a_block_uses_map_focus", async ({ page, engine, sent }) => {
    await openMap(page, await engine.loginUrl());
    const top = engineMap(engine);
    const chain = (top.levels ?? []).filter((l) => !l.apart);
    const below = chain[1]!.id;
    const block = top.nodes.find((n) => (n.counts[below] ?? 0) > 0)!;
    const bare = top.nodes.find((n) => (n.counts[below] ?? 0) === 0)!;
    const node = (id: string) =>
      page.locator(`.react-flow__node[data-id="${id}"]`);
    const mapAsked = (focus: string) =>
      page.waitForRequest(
        (r) =>
          r.method() === "POST" &&
          new URL(r.url()).pathname === "/api/v1/map" &&
          (r.postDataJSON() as { focus?: string }).focus === focus,
      );
    const lid = page.locator(".breadcrumb .lid-name");

    // A double press goes into the block: the engine opens the level below, so no level is asked.
    const dived = mapAsked(block.id);
    await doublePress(page, node(block.id), false);
    const body = (await dived).postDataJSON() as object;
    expect(body).not.toHaveProperty("level");
    await expect(lid).toHaveText(block.name);
    const inner = engineMap(engine, ["--focus", block.id]);
    await expect
      .poll(() => drawnIds(page))
      .toEqual(inner.nodes.map((n) => n.id).sort());

    // A block there that can dive shows an open lid, which goes in the same way.
    const innerChain = (inner.levels ?? []).filter((l) => !l.apart);
    const next =
      innerChain[innerChain.findIndex((l) => l.id === inner.level) + 1]!;
    const deeper = inner.nodes.find((n) => (n.counts[next.id] ?? 0) > 0)!;
    await node(deeper.id).click();
    const open = page.getByRole("button", {
      name: CANVAS_STRINGS.open_block.replace("{name}", deeper.name),
    });
    const again = mapAsked(deeper.id);
    await open.click();
    expect((await again).postDataJSON() as object).not.toHaveProperty("level");
    await expect(lid).toHaveText(deeper.name);
    await expect
      .poll(() => drawnIds(page))
      .toEqual(
        engineMap(engine, ["--focus", deeper.id])
          .nodes.map((n) => n.id)
          .sort(),
      );

    // The top lid brings back the top.
    await page
      .locator(".breadcrumb")
      .getByRole("button", { name: "shop", exact: true })
      .click();
    await expect(lid).toHaveText("shop");
    await expect
      .poll(() => drawnIds(page))
      .toEqual(top.nodes.map((n) => n.id).sort());

    // A block with nothing at the level below only selects, and offers no open lid.
    await expect.poll(() => settled(sent)).toBe(true);
    const asked = () =>
      sent.filter((r) => r.path === "/api/v1/map" && r.method === "POST")
        .length;
    const before = asked();
    await doublePress(page, node(bare.id), false);
    await expect(node(bare.id).locator(".block")).toHaveAttribute(
      "data-selected",
      "true",
    );
    await page.waitForTimeout(500);
    expect(asked()).toBe(before);
    await expect(
      page.getByRole("button", {
        name: CANVAS_STRINGS.open_block.replace("{name}", bare.name),
      }),
    ).toHaveCount(0);
  });

  test("an_edge_opens_its_evidence_from_ask_why", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    const chain = (engineMap(engine).levels ?? []).filter((l) => !l.apart);
    const level = chain.find(
      (l) => engineMap(engine, ["--level", l.id]).edges.length > 0,
    )!;
    const edge = engineMap(engine, ["--level", level.id]).edges[0]!;
    if (level !== chain[0]) {
      await page
        .locator(".rail")
        .getByRole("button", { name: level.label, exact: true })
        .click();
      await expect(page.locator(".breadcrumb .lid-name")).toHaveText(
        level.label,
      );
    }
    const why = engine.cli(["ask", "why", edge.from, edge.to]).result as {
      summary: string;
      items: { why: string; handles: { label: string; args: string[] }[] }[];
    };
    const handle = why.items[0]!.handles[0]!;
    const arg = (name: string) => handle.args[handle.args.indexOf(name) + 1];
    const sheet = page.getByRole("dialog", { name: CANVAS_STRINGS.evidence });
    const drawnEdge = page.locator(
      `.react-flow__edge[data-id^="${edge.from}>${edge.to}>"]`,
    );
    const open = async () => {
      await pressEdge(page, drawnEdge, false);
      await expect(sheet).toBeVisible();
    };

    await open();
    await expect(sheet.locator(".evidence-summary")).toHaveText(why.summary);
    await expect(sheet.locator(".evidence-why")).toHaveText(
      why.items.map((i) => i.why),
    );
    // A handle shows its view, focus included, and closes the sheet.
    const shown = page.waitForRequest(
      (r) =>
        r.method() === "POST" &&
        new URL(r.url()).pathname === "/api/v1/map" &&
        (r.postDataJSON() as { focus?: string }).focus === arg("--focus"),
    );
    await sheet.getByRole("button", { name: handle.label }).first().click();
    expect(((await shown).postDataJSON() as { level?: string }).level).toBe(
      arg("--level"),
    );
    await expect(sheet).toHaveCount(0);

    // Back at the edge, Escape closes the sheet.
    await page
      .locator(".breadcrumb")
      .getByRole("button", { name: "shop", exact: true })
      .click();
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("shop");
    await open();
    await page.keyboard.press("Escape");
    await expect(sheet).toHaveCount(0);
  });
  test("an_engine_refusal_keeps_the_map_and_shows_its_hint", async ({
    browser,
    engine,
  }) => {
    const { page, errors } = await ownPage(browser, false);
    try {
      const top = engineMap(engine);
      // A write the engine refuses with a hint and changes nothing: `edit` needs its base.
      const refused = engine.cli(["edit", "hide", top.nodes[0]!.id]);
      expect(refused.ok).toBe(false);
      expect(refused.error!.hint, "the engine's hint").toBeTruthy();
      await openMap(page, await engine.loginUrl());
      const before = await drawnIds(page);
      // The category usage maps to 400 (contracts.md, "HTTP status by category").
      await page.route(
        "**/api/v1/map",
        (route) =>
          route.fulfill({
            status: 400,
            contentType: "application/json",
            body: JSON.stringify(refused),
          }),
        { times: 1 },
      );
      const step = secondStep(engine);
      const rail = page.locator(".rail");
      await rail.getByRole("button", { name: step.label, exact: true }).click();
      const notices = page.locator(".notices");
      await expect(notices).toContainText(refused.error!.message);
      await expect(notices.locator(".empty-hint")).toHaveText(
        refused.error!.hint!,
      );
      // No full-page state: the same blocks, and the rail and breadcrumb still mark the top.
      await expect(page.locator("main.page")).toHaveCount(0);
      expect(await drawnIds(page)).toEqual(before);
      await expect(page.locator(".breadcrumb .lid-name")).toHaveText("shop");
      await expect(
        rail.locator("li:not([data-apart]) .rail-step").first(),
      ).toHaveAttribute("aria-current", /.+/);

      // The next press loads: the notice goes and the level shows.
      await rail.getByRole("button", { name: step.label, exact: true }).click();
      await expect(page.locator(".breadcrumb .lid-name")).toHaveText(
        step.label,
      );
      await expect(notices).toHaveCount(0);
      expect(
        errors.filter((e) => !e.includes("400")),
        "console and page errors other than the refused response",
      ).toEqual([]);
    } finally {
      await page.close();
    }
  });
});

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("exactly_five_concepts_on_the_phone", async ({
    browser,
    page,
    engine,
  }) => {
    test.setTimeout(180_000);
    await everyControlHasAConcept(browser, page, engine, true);
  });

  test("the_top_row_holds_four_targets_at_most_every_target_is_44px_and_nothing_overflows", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    // The `⋯` lid shows at the top too, where it reaches every level.
    await expect(
      page.getByRole("button", { name: CANVAS_STRINGS.levels_sheet }),
    ).toBeVisible();
    await topRowHolds(page);
    // Below the top the current lid overlaps it (ux-b).
    await page
      .locator(".sheet")
      .getByRole("button", { name: "Open the containers" })
      .first()
      .tap();
    await expect(page.locator(".breadcrumb .lid-name")).toHaveText("container");
    await expect(
      page.getByRole("button", { name: CANVAS_STRINGS.levels_sheet }),
    ).toBeVisible();
    await topRowHolds(page);
    // While search is open the lens switcher and the shelf yield: the row holds `⋯`, the input
    // and Close.
    await page.getByRole("button", { name: CANVAS_STRINGS.search }).tap();
    await expect(
      page.getByRole("searchbox", { name: CANVAS_STRINGS.search }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: CANVAS_STRINGS.lenses }),
    ).toHaveCount(0);
    await topRowHolds(page);
  });

  test("the_lens_switcher_and_the_project_shelf_offer_only_what_works_on_the_phone", async ({
    page,
    engine,
  }) => {
    await openMap(page, await engine.loginUrl());
    await offersOnlyWhatWorks(page, engine, true);
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
    await sheet.locator(".tray-badge").tap();
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
    const levels = (engineMap(engine).levels ?? []).map((l) => l.label);
    expect(levels.length).toBeGreaterThan(1);
    await expect(lid).toHaveText("shop");
    // The `⋯` lid shows at the top: its sheet lists every level of the shown map, the shown one
    // marked.
    const more = () =>
      page.getByRole("button", { name: CANVAS_STRINGS.levels_sheet });
    const dialog = page.getByRole("dialog", {
      name: CANVAS_STRINGS.levels_sheet,
    });
    await more().tap();
    await expect(dialog.locator("li[data-apart]")).toHaveCount(1);
    await expect(dialog.locator(".picker-entry")).toHaveText(levels);
    await expect(dialog.locator("[aria-current]")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await sheet.getByRole("button", { name: "Open the containers" }).tap();
    await expect(lid).toHaveText("container");
    // The sheet stops offering what it just did, and the row keeps four targets.
    await expect(
      sheet.getByRole("button", { name: "Open the containers" }),
    ).toHaveCount(0);
    expect(
      await page.locator(".top-row").locator("button, input").count(),
    ).toBeLessThanOrEqual(4);
    await more().tap();
    await expect(dialog.locator("[aria-current]")).toHaveText("container");
    await dialog.getByRole("button", { name: "shop" }).tap();
    await expect(lid).toHaveText("shop");
    await expect(
      sheet.getByRole("button", { name: "Open the containers" }).first(),
    ).toBeVisible();
  });

  test("search_opens_inside_the_lid", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    await page.getByRole("button", { name: CANVAS_STRINGS.search }).tap();
    await page
      .getByRole("searchbox", { name: CANVAS_STRINGS.search })
      .fill("kit");
    await expect(page.locator(".block:not([data-dim])")).toHaveCount(1);
  });

  test("the_phone_reaches_every_level", async ({ page, engine }) => {
    await openMap(page, await engine.loginUrl());
    const top = engineMap(engine);
    const levels = top.levels ?? [];
    const lid = page.locator(".breadcrumb .lid-name");
    const fits = async () => {
      expect(
        await lid.evaluate((e) => e.scrollWidth <= e.clientWidth),
        "the lid shows its whole name",
      ).toBe(true);
    };
    const more = () =>
      page.getByRole("button", { name: CANVAS_STRINGS.levels_sheet });
    const dialog = page.getByRole("dialog", {
      name: CANVAS_STRINGS.levels_sheet,
    });
    const topId = levels.find((l) => !l.apart)!.id;
    await topRowHolds(page);
    await fits();
    let shown = topId;
    for (const level of levels) {
      if (level.id === shown) {
        continue;
      }
      await more().tap();
      await dialog.getByRole("button", { name: level.label }).tap();
      await expect(lid).toHaveText(level.id === topId ? "shop" : level.label);
      shown = level.id;
      await topRowHolds(page);
      await fits();
    }

    // A double tap on a block goes into it; the top lid is back up through `⋯`.
    await more().tap();
    await dialog.getByRole("button", { name: "shop" }).tap();
    await expect(lid).toHaveText("shop");
    const chain = levels.filter((l) => !l.apart);
    const block = top.nodes.find((n) => (n.counts[chain[1]!.id] ?? 0) > 0)!;
    await doublePress(
      page,
      page.locator(`.react-flow__node[data-id="${block.id}"]`),
      true,
    );
    await expect(lid).toHaveText(block.name);
    await topRowHolds(page);
    await fits();

    // Search opens at the finest level too.
    await more().tap();
    await dialog.getByRole("button", { name: levels.at(-1)!.label }).tap();
    await expect(lid).toHaveText(levels.at(-1)!.label);
    await page.getByRole("button", { name: CANVAS_STRINGS.search }).tap();
    const box = page.getByRole("searchbox", { name: CANVAS_STRINGS.search });
    await expect(box).toBeVisible();
    await topRowHolds(page);
    await box.fill("a");
    await topRowHolds(page);
  });
});

/**
 * A picker with nothing to pick is flat: the shelf is a dashed outline with the blocks' radius
 * and no shadow, not a button, until a second project gives it two to pick between; the
 * Artifact's lens switcher is never a button.
 */
test("a_picker_with_nothing_to_pick_is_flat", async ({ page, engine }) => {
  await openMap(page, await engine.loginUrl());
  const shelf = page.locator(".top-row [data-concept=intent]");
  const flat = async () => {
    await expect(shelf.getByRole("button")).toHaveCount(0);
    const face = shelf.locator(".picker-face");
    const style = await face.evaluate((el) => {
      const probe = document.createElement("div");
      probe.style.borderRadius = "var(--radius-block)";
      document.body.append(probe);
      const block = getComputedStyle(probe).borderTopLeftRadius;
      probe.remove();
      const cs = getComputedStyle(el);
      return {
        border: cs.borderTopStyle,
        radius: cs.borderTopLeftRadius,
        block,
        shadow: cs.boxShadow,
        cursor: cs.cursor,
      };
    });
    expect(style.border).toBe("dashed");
    expect(style.radius).toBe(style.block);
    expect(style.shadow).toBe("none");
    expect(style.cursor).not.toBe("pointer");
  };
  const both = async (check: () => Promise<void>) => {
    for (const size of [DESKTOP, PHONE]) {
      await page.setViewportSize(size);
      await page.reload();
      await page.locator(".map[data-ready]").waitFor();
      await check();
    }
  };

  await both(flat);
  engine.cli(["intent", "new", "Checkout"], "agent:e2e:tester");
  await both(flat);
  engine.cli(["intent", "new", "Returns"], "agent:e2e:tester");
  await both(async () => {
    await expect(shelf.getByRole("button")).toHaveCount(1);
  });

  // On the Artifact page the lens switcher has nothing it can reach: no button, and a face with
  // no border or shadow.
  const ws = shareFixture();
  const host = await hostArtifact(ws.share);
  try {
    await openArtifact(page, ws, host, true);
    const lens = page.locator(".top-row [data-concept=lenses]");
    await both(async () => {
      await expect(lens.getByRole("button")).toHaveCount(0);
      const style = await lens.locator(".picker-face").evaluate((el) => {
        const cs = getComputedStyle(el);
        return {
          width: cs.borderTopWidth,
          style: cs.borderTopStyle,
          shadow: cs.boxShadow,
          cursor: cs.cursor,
        };
      });
      expect(style.style === "none" || style.width === "0px").toBe(true);
      expect(style.shadow).toBe("none");
      expect(style.cursor).not.toBe("pointer");
    });
  } finally {
    await host.close();
    ws.close();
  }
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

/**
 * The warnings `scan` carries, as the canvas shows a press's: each message once, in the engine's
 * order, three of them and then how many more. The workspace's own path differs between two
 * copies of it, so it is written `ROOT`.
 */
function scanWarnings(engine: Engine): { shown: string[]; more: number } {
  const lines = [
    ...new Set(
      (engine.cli(["scan"]).meta.warnings ?? []).map((w) =>
        w.message.split(engine.root).join("ROOT"),
      ),
    ),
  ];
  return { shown: lines.slice(0, 3), more: Math.max(0, lines.length - 3) };
}

test.describe("a workspace with repos a scan cannot map", () => {
  // The fixture's repos stay whole; one new repo has no commit and another git cannot read.
  const prepare = (root: string) => {
    initRepo(root, "newrepo");
    breakRepo(root, "broken");
  };

  test("per_repo_warnings_show", async ({ page }) => {
    const engine = await startEngine({ scanned: false, prepare });
    const twin = await startEngine({ scanned: false, prepare });
    try {
      const expected = scanWarnings(twin);
      expect(expected.shown.length, "warnings the scan gave").toBeGreaterThan(
        1,
      );
      await page.goto(await engine.loginUrl());
      await page.getByRole("button").click();
      await page.locator(".map[data-ready]").waitFor();
      const lines = page.locator(".notices li");
      const texts = async () =>
        (await lines.allTextContents()).map((t) =>
          t.split(engine.root).join("ROOT"),
        );
      const more =
        expected.more > 0
          ? [
              CANVAS_STRINGS.more_notices.replace(
                "{count}",
                String(expected.more),
              ),
            ]
          : [];
      await expect.poll(texts).toEqual([...expected.shown, ...more]);
      expect(expected.shown.join("\n")).toContain("newrepo");
      expect(expected.shown.join("\n")).toContain("broken");
    } finally {
      twin.close();
      engine.close();
    }
  });
});

// Repos with no commit cannot be mapped: the page says to commit, and once a repo has one the
// same button leads to the scan.
for (const viewport of [DESKTOP, PHONE]) {
  test(`an_unborn_only_workspace_says_to_commit at ${viewport.width}`, async ({
    page,
  }) => {
    const engine = await startEngine({
      scanned: false,
      prepare: (root) => {
        removeRepos(root);
        initRepo(root, "alpha");
        initRepo(root, "beta");
      },
    });
    try {
      await page.setViewportSize(viewport);
      const unborn = engineEmpty(engine)!;
      expect(unborn.reason).toBe("repos_unborn");
      await page.goto(await engine.loginUrl());
      const line = page.locator("[data-empty] .empty-line");
      const button = page.getByRole("button");
      await expect(line).toHaveText(unborn.line!);
      await expect(button).toHaveCount(1);
      await expect(button).toHaveText(unborn.next.label);

      // Pressing it asks again and finds the same thing.
      const asked = page.waitForResponse((r) =>
        r.url().endsWith("/api/v1/focus"),
      );
      await button.click();
      await asked;
      await expect(line).toHaveText(unborn.line!);
      await expect(button).toHaveCount(1);

      // After a commit the same button leads to the scan.
      engine.commit("alpha", { "README.md": "# alpha\n" }, "first commit");
      const ready = engineEmpty(engine)!;
      expect(ready.reason).toBe("not_scanned");
      await button.click();
      await expect(line).toHaveText(ready.line!);
      await expect(button).toHaveText(ready.next.label);
      await button.click();
      await page.locator(".map[data-ready]").waitFor();
      await expect(page.locator(".block")).toHaveCount(
        engineMap(engine).nodes.length,
      );
    } finally {
      engine.close();
    }
  });
}

for (const viewport of [DESKTOP, PHONE]) {
  test(`empty_state_audit at ${viewport.width}`, async ({ browser }) => {
    test.setTimeout(180_000);
    await everyEmptyStateHolds(browser, viewport === PHONE);
  });
}
