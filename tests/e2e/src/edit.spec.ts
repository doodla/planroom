/**
 * Hand edits on the canvas against the real engine (DR-68, DR-8): the action tray's buttons each
 * send one `edit` leaf with the tab's client ID, an `Idempotency-Key` equal to the request's
 * `idem` and the `seq` of the map drawn as `base`, and the engine's own map shows the result.
 */

import {
  CANVAS_STRINGS,
  type LiftedEdge,
  type MapNode,
  type MapView,
} from "@planroom/contract";
import type { Locator, Page } from "@playwright/test";

import { startEngine, type Engine, type ExportedEvent } from "./engine";
import {
  centreOf,
  DESKTOP,
  dragTo,
  editBodies,
  expect,
  goToLevel,
  openMap,
  openTray,
  PHONE,
  pressEdge,
  test,
} from "./fixtures";

/**
 * The engine's whole map, as the canvas asks for it: the CLI keeps each reply to its budget and
 * pages, so the pages are followed to the end and their blocks and edges joined.
 */
function graph(engine: Engine, more: readonly string[] = []): MapView {
  const args = ["map", "--detail", "graph", ...more];
  const first = engine.cli(args);
  const view = first.result as MapView;
  let next = first.meta.next;
  while (next) {
    const page = engine.cli([...args, "--cursor", next]);
    const rest = page.result as MapView;
    view.nodes.push(...rest.nodes);
    const seen = new Set(view.edges.map((e) => `${e.kind} ${e.from} ${e.to}`));
    view.edges.push(
      ...rest.edges.filter((e) => !seen.has(`${e.kind} ${e.from} ${e.to}`)),
    );
    next = page.meta.next;
  }
  return view;
}

/** A block's place on the felt in the view's own coordinates, from its node's transform. */
async function placeOf(
  page: Page,
  id: string,
): Promise<{ x: number; y: number }> {
  return page.locator(`.react-flow__node[data-id="${id}"]`).evaluate((el) => {
    const m = new DOMMatrix(getComputedStyle(el).transform);
    return { x: m.e, y: m.f };
  });
}

/** The non-apart levels of the chain the engine reads, top first. */
/** An event's kind, the tag of its body, as the data root's event backup holds it. */
const kindOf = (e: ExportedEvent): unknown =>
  (e.body as { kind?: unknown } | undefined)?.kind;

const stepsOf = (view: MapView) => (view.levels ?? []).filter((l) => !l.apart);

/**
 * One press on a lifted edge where the page really shows it: along its path, the first point that
 * is inside the screen and whose topmost element belongs to the edge; false when the edge has
 * none. On the phone the felt is wider than the screen, so an edge may be off it.
 */
async function pressEdgeInView(
  page: Page,
  edge: Locator,
  phone: boolean,
): Promise<boolean> {
  const at = await edge
    .locator("path.react-flow__edge-interaction")
    .evaluate((el) => {
      const path = el as unknown as SVGPathElement;
      const m = path.getScreenCTM()!;
      const total = path.getTotalLength();
      for (let step = 1; step < 40; step++) {
        const p = path.getPointAtLength((total * step) / 40);
        const x = m.a * p.x + m.c * p.y + m.e;
        const y = m.b * p.x + m.d * p.y + m.f;
        const top = document.elementFromPoint(x, y);
        if (top !== null && path.parentElement!.contains(top)) {
          return { x, y };
        }
      }
      return null;
    });
  if (at === null) {
    return false;
  }
  if (phone) {
    await page.touchscreen.tap(at.x, at.y);
  } else {
    await page.mouse.click(at.x, at.y);
  }
  return true;
}

/** A lifted edge as the canvas draws it: its ID is `from>to>kind`. */
const edgeOf = (page: Page, e: LiftedEdge) =>
  page.getByTestId(`rf__edge-${e.from}>${e.to}>${e.kind}`);

const nodeOf = (page: Page, id: string) =>
  page.locator(`.react-flow__node[data-id="${id}"]`);

/**
 * Two presses on the part of a block the screen shows: on the phone the felt is wider than the
 * screen, so the middle of a block at its edge may be off it.
 */
async function diveInto(page: Page, id: string, phone: boolean) {
  const box = (await nodeOf(page, id).boundingBox())!;
  const view = page.viewportSize()!;
  const left = Math.max(box.x, 0);
  const right = Math.min(box.x + box.width, view.width);
  const at = { x: (left + right) / 2, y: box.y + box.height / 2 };
  if (phone) {
    await page.touchscreen.tap(at.x, at.y);
    await page.touchscreen.tap(at.x, at.y);
  } else {
    await page.mouse.dblclick(at.x, at.y);
  }
}

/**
 * Dive from the top into storefront, then go to the component level, which draws its directories
 * as blocks, and read the blocks that level holds from the engine: the names are the fixture's,
 * the IDs the engine's.
 */
async function intoStorefront(page: Page, engine: Engine, phone: boolean) {
  const top = graph(engine);
  const store = top.nodes.find((n) => n.name === "storefront")!;
  const level = stepsOf(top)[2]!.id;
  const inside = () =>
    (
      engine.cli([
        "map",
        "--detail",
        "graph",
        "--level",
        level,
        "--focus",
        store.id,
      ]).result as MapView
    ).nodes;
  const named = (name: string) => inside().find((n) => n.name === name)!;
  await diveInto(page, store.id, phone);
  await expect(page.locator(".breadcrumb .lid-name").last()).toHaveText(
    store.name,
  );
  await goToLevel(page, stepsOf(top)[2]!.label, phone);
  await expect(nodeOf(page, named("cart").id)).toBeVisible();
  return { named, inside };
}

/**
 * Zoom the felt out, a wheel notch at a time, until every one of `blocks` sits well inside the map,
 * clear of the edges where a drag would pan the felt: a new group lands wherever the layout puts
 * it, which the view drawn before it may not cover.
 */
async function zoomToShow(page: Page, blocks: Locator[]) {
  const map = (await page.locator(".map").boundingBox())!;
  const margin = 120;
  const inside = async (block: Locator) => {
    const box = (await block.boundingBox())!;
    return (
      box.x >= map.x + margin &&
      box.y >= map.y + margin &&
      box.x + box.width <= map.x + map.width - margin &&
      box.y + box.height <= map.y + map.height - margin
    );
  };
  await page.mouse.move(map.x + map.width / 2, map.y + map.height / 2);
  for (let notch = 0; notch < 12; notch++) {
    const shown = await Promise.all(blocks.map(inside));
    if (shown.every(Boolean)) {
      return;
    }
    await page.mouse.wheel(0, 200);
    await page.waitForTimeout(100);
  }
}

/** Group `first` with `second` under `name` from `first`'s tray. */
async function groupFromTray(
  page: Page,
  phone: boolean,
  first: string,
  second: { id: string; name: string },
  name: string,
) {
  const tray = await openTray(page, first, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.group_with }).click();
  await tray.getByRole("checkbox", { name: second.name }).check();
  await tray.getByRole("button", { name: CANVAS_STRINGS.done }).click();
  const field = page.getByLabel(CANVAS_STRINGS.name_field);
  await field.fill(name);
  await field.press("Enter");
}

/**
 * The blocks held by the group or block `id`, as the engine lists them when it centres on `id`:
 * the level below a block draws the blocks inside its members, so each member is named by the
 * parent of what the engine lists.
 */
const heldBy = (engine: Engine, id: string): string[] => [
  ...new Set(
    (
      engine.cli(["map", "--focus", id, "--detail", "graph"]).result as MapView
    ).nodes.map((n) => n.parent ?? ""),
  ),
];

/**
 * Every `edit` request carries the tab's client ID, its own `Idempotency-Key` and a `base`. The
 * headers reach `sent` a moment after the requests do, so it waits for them.
 */
async function everyEditIsStamped(sent: Parameters<typeof editBodies>[0]) {
  await expect
    .poll(() => editBodies(sent).every((e) => e.idemKey !== undefined))
    .toBe(true);
  for (const e of editBodies(sent)) {
    expect(e.body.client, "the tab's client ID").toMatch(/^tab:[0-9a-f]{16}$/);
    expect(e.idemKey, "the Idempotency-Key header").toBe(e.body.idem);
    expect(e.body.idem).toBeTruthy();
    expect(typeof e.body.base).toBe("number");
  }
}

/**
 * Seven hand edits, one of each kind, then each one undone once from the list of this tab's edits: the top level
 * takes the rename, the hide, the pin, the relation and the forbid; storefront's directories take the
 * group and the move.
 */
async function eachHandEditUndoes(
  page: Page,
  engine: Engine,
  sent: Parameters<typeof editBodies>[0],
  phone: boolean,
) {
  const top = graph(engine);
  const byName = (name: string) => top.nodes.find((n) => n.name === name)!;
  // Blocks the top row of the map doesn't cover: those get the rename, the hide and the pin.
  const [renamed, hidden, pinned, worker, orders] = [
    byName("orders-db"),
    byName("redis"),
    byName("otel-collector"),
    byName("worker"),
    byName("orders"),
  ];
  const felt = () =>
    page.locator(".react-flow__pane").click({ position: { x: 4, y: 4 } });
  await openMap(page, await engine.loginUrl());

  // Rename, hide and pin, from their trays.
  let tray = await openTray(page, renamed.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.rename }).click();
  const field = page.getByLabel(CANVAS_STRINGS.name_field);
  await field.fill("hand renamed");
  await field.press("Enter");
  await expect(
    page.locator(".block-name", { hasText: "hand renamed" }),
  ).toBeVisible();
  tray = await openTray(page, hidden.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.hide }).click();
  await expect(nodeOf(page, hidden.id)).toHaveCount(0);
  tray = await openTray(page, pinned.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.pin }).click();

  // Relate to: the kinds are the ones the view draws.
  tray = await openTray(page, worker.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.relate_to }).click();
  await tray
    .getByRole("list", { name: CANVAS_STRINGS.pick_block })
    .getByRole("button", { name: orders.name, exact: true })
    .click();
  if (new Set(top.edges.map((e) => e.kind)).size > 1) {
    await tray
      .getByRole("list", { name: CANVAS_STRINGS.pick_relation })
      .getByRole("button")
      .first()
      .click();
  }
  await felt();

  // Forbid, from the evidence sheet of an edge the screen shows between two blocks the other edits
  // leave be.
  let edge: LiftedEdge | undefined;
  for (const candidate of top.edges) {
    if (candidate.from === hidden.id || candidate.to === hidden.id) {
      continue;
    }
    if (await pressEdgeInView(page, edgeOf(page, candidate), phone)) {
      edge = candidate;
      break;
    }
  }
  await page
    .getByRole("button", {
      name: CANVAS_STRINGS.forbid.replace("{kind}", edge!.kind),
    })
    .click();
  await felt();
  await expect
    .poll(() => editBodies(sent).map((e) => e.body.op))
    .toEqual(["rename", "hide", "pin", "relate", "forbid"]);

  // Group and move, inside storefront.
  const { named, inside } = await intoStorefront(page, engine, phone);
  const [cart, checkout, catalog] = [
    named("cart"),
    named("checkout"),
    named("catalog"),
  ];
  await groupFromTray(page, phone, cart.id, checkout, "purchase");
  await expect
    .poll(() => inside().find((n) => n.name === "purchase")?.id)
    .toBeDefined();
  const purchase = inside().find((n) => n.name === "purchase")!;
  await expect(nodeOf(page, purchase.id)).toBeVisible();
  if (phone) {
    tray = await openTray(page, catalog.id, true);
    await tray.getByRole("button", { name: CANVAS_STRINGS.move_into }).click();
    await tray
      .getByRole("list", { name: CANVAS_STRINGS.pick_block })
      .getByRole("button", { name: "purchase", exact: true })
      .click();
  } else {
    await zoomToShow(page, [
      nodeOf(page, purchase.id),
      nodeOf(page, catalog.id),
    ]);
    await dragTo(
      page,
      nodeOf(page, catalog.id),
      await centreOf(nodeOf(page, purchase.id)),
    );
  }
  await expect
    .poll(() => editBodies(sent).map((e) => e.body.op))
    .toHaveLength(7);
  await expect.poll(() => heldBy(engine, purchase.id)).toContain(catalog.id);
  const made = editBodies(sent);
  expect(made.map((e) => e.body.op).sort()).toEqual(
    ["forbid", "group", "hide", "move", "pin", "relate", "rename"].sort(),
  );

  // Undo each, oldest first, from the list: a new row lands on top each time, so the next oldest
  // edit stays seven rows from the top until the list's cap.
  await felt();
  if (phone) {
    await page.getByRole("button", { name: CANVAS_STRINGS.sheet_half }).tap();
  } else {
    await page.locator(".drawer .tray-badge").click();
  }
  const rows = page.locator(".edit-list .edit-row");
  await expect(rows).toHaveCount(7);
  for (let i = 0; i < 7; i++) {
    const row = rows.nth(6);
    await expect(row).toHaveAttribute("data-state", "done");
    await row.getByRole("button", { name: CANVAS_STRINGS.undo }).click();
    await expect
      .poll(() => editBodies(sent).filter((e) => e.body.op === "undo").length)
      .toBe(i + 1);
    await expect(rows.nth(0)).toHaveAttribute("data-state", "done");
  }
  const undos = editBodies(sent).filter((e) => e.body.op === "undo");
  await everyEditIsStamped(sent);
  const idems = [...made, ...undos].map((e) => e.body.idem);
  expect(new Set(idems).size, "each edit has its own idem").toBe(14);

  // Nothing of them is in effect: the map reads as before, and the engine wrote one undo event
  // for each command.
  const after = graph(engine);
  expect(after.nodes.find((n) => n.id === renamed.id)?.name).toBe(renamed.name);
  expect(after.hidden).toBe(top.hidden);
  expect(inside().map((n) => n.name)).not.toContain("purchase");
  expect(inside().map((n) => n.id)).toEqual(
    expect.arrayContaining([cart.id, checkout.id, catalog.id]),
  );
  await expect
    .poll(() => engine.exported().filter((e) => kindOf(e) === "undo"))
    .toHaveLength(7);
}

async function renameHidePin(page: Page, engine: Engine, phone: boolean) {
  const nodes = graph(engine).nodes.filter((n) => n.parent == null);
  const [renamed, hidden, pinned] = [nodes[0]!, nodes[1]!, nodes[2]!];
  await openMap(page, await engine.loginUrl());

  // Rename, through the tray's field.
  let tray = await openTray(page, renamed.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.rename }).click();
  const field = page.getByLabel(CANVAS_STRINGS.name_field);
  await expect(field).toBeFocused();
  await field.fill("hand renamed");
  await field.press("Enter");
  await expect(
    page.locator(".block-name", { hasText: "hand renamed" }),
  ).toBeVisible();
  await expect
    .poll(() => graph(engine).nodes.find((n) => n.id === renamed.id)?.name)
    .toBe("hand renamed");

  // Hide another block: it leaves the map and the view counts it.
  const before = graph(engine).hidden;
  tray = await openTray(page, hidden.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.hide }).click();
  await expect(
    page.locator(`.react-flow__node[data-id="${hidden.id}"]`),
  ).toHaveCount(0);
  await expect.poll(() => graph(engine).hidden).toBe(before + 1);

  // Pin a third where it sits; a reload brings it back to the same place.
  const at = await placeOf(page, pinned.id);
  tray = await openTray(page, pinned.id, phone);
  await tray.getByRole("button", { name: CANVAS_STRINGS.pin }).click();
  await expect
    .poll(() => graph(engine).nodes.find((n) => n.id === pinned.id)?.position)
    .toBeDefined();
  await openMap(page, await engine.loginUrl());
  const after = await placeOf(page, pinned.id);
  expect(Math.abs(after.x - at.x)).toBeLessThanOrEqual(1);
  expect(Math.abs(after.y - at.y)).toBeLessThanOrEqual(1);
}

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("rename_hide_and_pin_from_the_tray", async ({ page, engine, sent }) => {
    await renameHidePin(page, engine, false);
    const edits = editBodies(sent);
    expect(edits.map((e) => e.body.op)).toEqual(["rename", "hide", "pin"]);
    for (const e of edits) {
      expect(e.body.client, "the tab's client ID").toMatch(
        /^tab:[0-9a-f]{16}$/,
      );
      expect(e.idemKey, "the Idempotency-Key header").toBe(e.body.idem);
      expect(e.body.idem).toBeTruthy();
      expect(typeof e.body.base).toBe("number");
    }
  });

  test("regroup_by_a_lid_drop_and_by_the_tray", async ({
    page,
    engine,
    sent,
  }) => {
    await openMap(page, await engine.loginUrl());
    const { named, inside } = await intoStorefront(page, engine, false);
    const [cart, checkout, catalog] = [
      named("cart"),
      named("checkout"),
      named("catalog"),
    ];

    // Group two blocks from the tray.
    await groupFromTray(page, false, cart.id, checkout, "purchase");
    await expect
      .poll(() => inside().find((n) => n.name === "purchase")?.id)
      .toBeDefined();
    const purchase = inside().find((n) => n.name === "purchase")!;
    expect(heldBy(engine, purchase.id)).toEqual(
      expect.arrayContaining([cart.id, checkout.id]),
    );

    // Drop a third block on the group's lid: it raises while the block is over it.
    const target = nodeOf(page, purchase.id);
    await expect(target).toBeVisible();
    await zoomToShow(page, [target, nodeOf(page, catalog.id)]);
    await dragTo(page, nodeOf(page, catalog.id), await centreOf(target), () =>
      expect(target.locator(".block")).toHaveAttribute(
        "data-drop-target",
        "true",
      ),
    );
    await expect.poll(() => heldBy(engine, purchase.id)).toContain(catalog.id);

    // The page's requests reach `sent` as events, which a blocking engine read holds back.
    await expect
      .poll(() => editBodies(sent).map((e) => e.body.op))
      .toEqual(["group", "move"]);
    await everyEditIsStamped(sent);
  });

  test("chalk_makes_an_intended_relation_and_forbid", async ({
    page,
    engine,
    sent,
  }) => {
    const step = stepsOf(graph(engine))[1]!;
    const view = graph(engine, ["--level", step.id]);
    const worker = view.nodes.find((n) => n.name === "worker")!;
    const orders = view.nodes.find((n) => n.name === "orders")!;
    const kinds = new Set(view.edges.map((e) => e.kind));
    await openMap(page, await engine.loginUrl());
    await goToLevel(page, step.label, false);

    // Chalk from worker to orders; the relation is picked when the view draws several kinds.
    await nodeOf(page, worker.id).click();
    const chalk = nodeOf(page, worker.id).locator(".chalk-handle");
    await expect(chalk).toBeVisible();
    await dragTo(page, chalk, await centreOf(nodeOf(page, orders.id)));
    const pick = page.getByRole("list", { name: CANVAS_STRINGS.pick_relation });
    if (kinds.size > 1) {
      await pick.getByRole("button").first().click();
    }
    await expect
      .poll(() => editBodies(sent).map((e) => e.body.op))
      .toEqual(["relate"]);
    const relate = editBodies(sent)[0]!.body;
    expect([relate.from, relate.to]).toEqual([worker.id, orders.id]);
    expect(kinds.has(relate.kind as string)).toBe(true);

    // Forbid, from the evidence sheet of an edge the view draws.
    const edge = view.edges[0]!;
    await pressEdge(page, edgeOf(page, edge), false);
    const forbid = page.getByRole("button", {
      name: CANVAS_STRINGS.forbid.replace("{kind}", edge.kind),
    });
    await forbid.click();
    await expect
      .poll(() => editBodies(sent).map((e) => e.body.op))
      .toEqual(["relate", "forbid"]);
    await everyEditIsStamped(sent);

    await expect
      .poll(() => engine.exported().filter((e) => kindOf(e) !== "scan"), {
        message: "events",
      })
      .toHaveLength(2);
  });

  test("each_hand_edit_undoes", async ({ page, engine, sent }) => {
    await eachHandEditUndoes(page, engine, sent, false);
  });

  test("one_tab_from_the_selected_block_reaches_the_tray", async ({
    page,
    engine,
  }) => {
    const block = graph(engine).nodes[0]!;
    await openMap(page, await engine.loginUrl());
    const tray = await openTray(page, block.id, false);
    await page.locator(`.react-flow__node[data-id="${block.id}"]`).focus();
    await page.keyboard.press("Tab");
    await expect(
      tray.getByRole("button", { name: CANVAS_STRINGS.rename }),
    ).toBeFocused();
  });
  test("the_first_scan_from_the_empty_state_offers_hand_edits", async ({
    page,
    sent,
  }) => {
    // No store yet: the session reply names no workspace, so only the scan's reply can.
    const engine = await startEngine({ scanned: false });
    try {
      await page.goto(await engine.loginUrl());
      await page
        .getByRole("button", { name: "Map these 5 repos? About 2 s." })
        .click();
      await page.locator(".map[data-ready]").waitFor();
      const block = graph(engine).nodes.find((n) => n.parent == null)!;
      const tray = await openTray(page, block.id, false);
      await tray.getByRole("button", { name: CANVAS_STRINGS.rename }).click();
      const field = page.getByLabel(CANVAS_STRINGS.name_field);
      await field.fill("named after the first scan");
      await field.press("Enter");
      await expect
        .poll(() => graph(engine).nodes.find((n) => n.id === block.id)?.name)
        .toBe("named after the first scan");
      await everyEditIsStamped(sent);
      expect(editBodies(sent).map((e) => e.body.op)).toEqual(["rename"]);
    } finally {
      engine.close();
    }
  });
});

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("rename_hide_and_pin_from_the_tray", async ({ page, engine, sent }) => {
    await renameHidePin(page, engine, true);
    const edits = editBodies(sent);
    expect(edits.map((e) => e.body.op)).toEqual(["rename", "hide", "pin"]);
    await everyEditIsStamped(sent);
  });

  test("each_hand_edit_undoes", async ({ page, engine, sent }) => {
    await eachHandEditUndoes(page, engine, sent, true);
  });

  test("the_phone_tray_moves_and_relates", async ({ page, engine, sent }) => {
    const top = graph(engine);
    const [a, b, c] = top.nodes.filter((n) => n.parent == null) as [
      MapNode,
      MapNode,
      MapNode,
    ];
    await openMap(page, await engine.loginUrl());

    // A one-finger drag on a block pans the felt and moves nothing.
    const viewport = page.locator(".react-flow__viewport");
    const before = await viewport.getAttribute("style");
    const at = await centreOf(nodeOf(page, a.id));
    const touch = await page.context().newCDPSession(page);
    const finger = (dx: number) => [{ x: at.x + dx, y: at.y }];
    await touch.send("Input.dispatchTouchEvent", {
      type: "touchStart",
      touchPoints: finger(0),
    });
    for (let dx = 10; dx <= 80; dx += 10) {
      await touch.send("Input.dispatchTouchEvent", {
        type: "touchMove",
        touchPoints: finger(dx),
      });
    }
    await touch.send("Input.dispatchTouchEvent", {
      type: "touchEnd",
      touchPoints: [],
    });
    await expect.poll(() => viewport.getAttribute("style")).not.toBe(before);
    expect(editBodies(sent)).toEqual([]);

    // A long press selects the block and opens the sheet to half with the tray first in it.
    const opened = await openTray(page, a.id, true);
    await expect(page.locator(".sheet")).toHaveAttribute("data-detent", "half");
    await expect(page.locator(".sheet-body > *").first()).toHaveAttribute(
      "role",
      "toolbar",
    );

    // The sheet's own toggle says it is open and takes it back to peek, the tray with it.
    const toggle = page.getByRole("button", {
      name: CANVAS_STRINGS.sheet_peek,
    });
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await toggle.click();
    await expect(page.locator(".sheet")).toHaveAttribute("data-detent", "peek");
    await expect(opened).toHaveCount(0);
    const tray = await openTray(page, a.id, true);

    // Move into, from its list.
    await tray.getByRole("button", { name: CANVAS_STRINGS.move_into }).click();
    await tray
      .getByRole("list", { name: CANVAS_STRINGS.pick_block })
      .getByRole("button", { name: b.name, exact: true })
      .click();
    await expect
      .poll(() => editBodies(sent).map((e) => e.body.op))
      .toEqual(["move"]);

    // Relate to, from its list, and the relation when the view draws several kinds.
    const again = await openTray(page, b.id, true);
    await again.getByRole("button", { name: CANVAS_STRINGS.relate_to }).click();
    await again
      .getByRole("list", { name: CANVAS_STRINGS.pick_block })
      .getByRole("button", { name: c.name, exact: true })
      .click();
    if (new Set(top.edges.map((e) => e.kind)).size > 1) {
      await again
        .getByRole("list", { name: CANVAS_STRINGS.pick_relation })
        .getByRole("button")
        .first()
        .click();
    }
    await expect
      .poll(() => editBodies(sent).map((e) => e.body.op))
      .toEqual(["move", "relate"]);
    await everyEditIsStamped(sent);
  });
});
