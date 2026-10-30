/**
 * The canvas published as an Artifact (S11, the walking skeleton's WS7): `planroom share`'s real
 * bundle on a host that serves it the way the platform does, with `tests/e2e/shims/` standing in
 * for `window.claude` and its `db`. The round trip: the page draws the snapshot, a dive the snapshot
 * doesn't hold becomes a read request row in the viewer's own inbox, the real built
 * `planroom import -` answers it from that row, the session's batch lands in the db, and the page
 * draws the answer. Every test also fails on a console error, a CSP violation among them.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import type { Page } from "@playwright/test";
import {
  CANVAS_STRINGS,
  type InboxRow,
  type MapView,
  type Snapshot,
} from "@planroom/contract";

import type { StoredDoc } from "../shims/artifact-db";
import {
  batchesOf,
  hostArtifact,
  importRows,
  inline,
  openArtifact,
  shareFixture,
  type ArtifactHostServer,
  type SharedWorkspace,
} from "./artifact-host";
import { CONCEPTS, DESKTOP, PHONE, expect, test } from "./fixtures";

const UID = "u_test";
const ASK = "Not in this snapshot. Ask your computer for it";

let ws: SharedWorkspace;
let host: ArtifactHostServer;

test.beforeAll(async () => {
  ws = shareFixture();
  host = await hostArtifact(ws.share);
});

test.afterAll(async () => {
  await host.close();
  ws.close();
});

/** Open the Artifact as a contributor, with `live/head` as the session's first batch left it. */
function open(page: Page, db: boolean): Promise<void> {
  return openArtifact(page, ws, host, db);
}

const session = {
  list: (page: Page, collection: string) =>
    page.evaluate((c) => window.__artifactDb!.session.list(c), collection),
  get: (page: Page, path: string) =>
    page.evaluate((p) => window.__artifactDb!.session.get(p), path),
};

/** The snapshot `share` published. */
function snapshot(): Snapshot {
  const publish = ws.share.publish;
  const path = join(publish.root, publish.files["data/snapshot.json"]!);
  return JSON.parse(readFileSync(path, "utf8")) as Snapshot;
}

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("a_view_outside_the_snapshot_becomes_a_request", async ({ page }) => {
    await open(page, true);
    const top = snapshot().maps[0]!;
    await expect(page.locator(".block")).toHaveCount(top.nodes.length);
    // The page fetched its snapshot by relative path from its own files.
    expect(host.requested).toContain("data/snapshot.json");

    // Dive: the snapshot holds only the top level, so the view asks for itself.
    await page.locator(".rail-step", { hasText: "container" }).click();
    const request = page.locator(".request-state");
    await expect(request).toHaveAttribute("data-request", "unavailable");
    await page.getByRole("button", { name: ASK }).click();
    await expect(request).toHaveAttribute("data-request", "asked");
    await expect(request).toContainText(CANVAS_STRINGS.asked);
    await expect(
      page.getByRole("button", { name: CANVAS_STRINGS.back_to_map }),
    ).toBeVisible();

    // The row sits in the viewer's own inbox, and its head was bumped.
    const rows: StoredDoc[] = await session.list(page, `inbox/${UID}/events`);
    expect(rows).toHaveLength(1);
    const row = rows[0]!.data as unknown as InboxRow;
    expect(row).toMatchObject({
      format: "planroom.inbox/v1",
      counter: 1,
      command: "map",
      request: { detail: "graph", level: "container" },
    });
    expect(await session.get(page, `inbox/${UID}`)).toMatchObject({
      data: { seq: 1 },
    });

    // The session's sync: the observed list result, as {path, row, version} lines on stdin, then
    // the batch import returns, written as ArtifactData writes them.
    const head = (await session.get(page, "live/head"))!;
    const result = importRows(ws, [
      { path: rows[0]!.path, row: rows[0]!.data, version: rows[0]!.version },
      { path: "live/head", version: head.version },
    ]);
    expect(result.person).toBe(false);
    expect(result.rows).toEqual([
      { path: rows[0]!.path, command: "map", status: "imported", reason: null },
    ]);
    for (const batch of batchesOf(result)) {
      const writes = inline(batch);
      await page.evaluate((w) => window.__artifactDb!.session.batch(w), writes);
    }

    // The page draws the answer: the engine's own map at that level.
    const engine = ws.cli(["map", "--level", "container", "--detail", "graph"])
      .result as MapView;
    await expect(page.locator(".request-state")).toHaveCount(0);
    await page.locator(".map[data-ready]").waitFor();
    await expect(page.locator(".block")).toHaveCount(engine.nodes.length);
    expect(
      (await page.locator(".block-name").allTextContents()).sort(),
    ).toEqual(engine.nodes.map((n) => n.name).sort());
    // The row was acked and deleted, and live/head moved on.
    expect(await session.list(page, `inbox/${UID}/events`)).toEqual([]);
    const docId = rows[0]!.path.split("/").at(-1)!;
    expect((await session.get(page, `acks/${UID}`))?.data).toMatchObject({
      rows: { [docId]: "imported" },
    });
    expect((await session.get(page, "live/head"))?.version).toBe(2);

    // Back to the map keeps working once the answer is in.
    await page.locator(".rail-step", { hasText: "system" }).click();
    await expect(page.locator(".block")).toHaveCount(top.nodes.length);
  });

  test("the_page_renders_the_snapshot_without_a_db", async ({ page }) => {
    await open(page, false);
    await expect(page.locator(".block")).toHaveCount(
      snapshot().maps[0]!.nodes.length,
    );
    await expect(page.locator(".transport-note")).toHaveText(
      CANVAS_STRINGS.snapshot_only,
    );
    const concepts = await page
      .locator("[data-concept]")
      .evaluateAll((els) => [
        ...new Set(els.map((e) => e.getAttribute("data-concept"))),
      ]);
    expect(concepts.sort()).toEqual(CONCEPTS);
    // With no db there's nothing to ask: the line, and the way back.
    await page.locator(".rail-step", { hasText: "container" }).click();
    await expect(page.locator(".request-state")).toContainText(ASK);
    await expect(page.getByRole("button", { name: ASK })).toHaveCount(0);
    await page
      .getByRole("button", { name: CANVAS_STRINGS.back_to_map })
      .click();
    await page.locator(".map[data-ready]").waitFor();
    await expect(page.locator(".block")).toHaveCount(
      snapshot().maps[0]!.nodes.length,
    );
  });

  test("the_fake_db_keeps_a_contributor_to_its_own_inbox", async ({ page }) => {
    await open(page, true);
    const outcome = await page.evaluate(async () => {
      const db = (await window.claude.use("db")) as {
        doc(p: string): {
          set(d: object): Promise<void>;
          get(): Promise<{ exists: boolean }>;
        };
      };
      const own = await db
        .doc("inbox/u_test/events/a")
        .set({ x: 1 })
        .then(
          () => "ok",
          (e: { code: string }) => e.code,
        );
      const other = await db
        .doc("inbox/u_other/events/a")
        .set({ x: 1 })
        .then(
          () => "ok",
          (e: { code: string }) => e.code,
        );
      const shared = await db
        .doc("live/head")
        .set({ x: 1 })
        .then(
          () => "ok",
          (e: { code: string }) => e.code,
        );
      return { own, other, shared };
    });
    expect(outcome).toEqual({
      own: "ok",
      other: "invalid_argument",
      shared: "invalid_argument",
    });
  });
});

test.describe("at 390x844", () => {
  test.use({ viewport: PHONE, hasTouch: true });

  test("the_next_action_card_dives_into_a_request_on_a_phone", async ({
    page,
  }) => {
    await open(page, true);
    // The phone has no rail: the face-up card's next action opens the containers.
    await page.getByRole("button", { name: "Open the containers" }).tap();
    await expect(page.locator(".request-state")).toHaveAttribute(
      "data-request",
      "unavailable",
    );
    await page.getByRole("button", { name: ASK }).tap();
    await expect(page.locator(".request-state")).toHaveAttribute(
      "data-request",
      "asked",
    );
    expect(await session.list(page, `inbox/${UID}/events`)).toHaveLength(1);
    await page.getByRole("button", { name: CANVAS_STRINGS.back_to_map }).tap();
    await page.locator(".map[data-ready]").waitFor();
  });
});
