/**
 * S1's first look on the real engine: the tones and lifted edges the canvas draws on
 * `planroom serve`, and the block counts as the ladder's words. Every expected value comes from the
 * engine's own `map --detail graph` reply, so a fixture or linker change needs no edit here.
 */

import { readFileSync } from "node:fs";

import {
  CANVAS_STRINGS,
  type LiftedEdge,
  type MapNode,
  type MapView,
} from "@planroom/contract";
import type { Locator, Page } from "@playwright/test";

import type { Engine } from "./engine";
import {
  DESKTOP,
  PHONE,
  expect,
  lensItems,
  openMap,
  test,
  tokenColor,
} from "./fixtures";

/**
 * The engine's whole map of the default lens (or `lens`) at the top level (or `level`), as the
 * canvas reads it over HTTP: the CLI keeps each reply to its budget and pages, so the pages are
 * followed to the end and their blocks and edges joined.
 */
function engineMap(
  engine: Engine,
  { lens, level }: { lens?: string; level?: string } = {},
): MapView {
  const args = ["map", "--detail", "graph"];
  if (lens) {
    args.push("--lens", lens);
  }
  if (level) {
    args.push("--level", level);
  }
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

/** `code.queue_publish` as `queue publish`. */
function words(kind: string): string {
  const short = kind.includes(".") ? kind.slice(kind.indexOf(".") + 1) : kind;
  return short.replaceAll("_", " ");
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
 * The default lens's ID and yarn as the code pack declares them, read from its `pack.toml` rather
 * than from the engine, so a view that names the wrong lens's yarn fails.
 */
function packDefaultLens(): { id: string; yarn: string } {
  const toml = readFileSync(
    new URL("../../../packs/code/pack.toml", import.meta.url),
    "utf8",
  );
  const lenses = toml.split(/^\[\[/m).filter((b) => b.startsWith("lenses]]"));
  const value = (block: string, key: string) =>
    new RegExp(`^${key}\\s*=\\s*"?([^"\\n]+?)"?\\s*$`, "m").exec(block)?.[1];
  const defaults = lenses.filter((b) => value(b, "default") === "true");
  expect(defaults, "the code pack's one default lens").toHaveLength(1);
  const [id, yarn] = [value(defaults[0]!, "id"), value(defaults[0]!, "yarn")];
  expect(id, "the default lens's id").toBeDefined();
  expect(yarn, "the default lens's yarn").toBeDefined();
  return { id: id!, yarn: yarn! };
}

/** The edges with both ends among the view's blocks. */
function onMap(view: MapView): LiftedEdge[] {
  const ids = new Set(view.nodes.map((n) => n.id));
  return view.edges.filter((e) => ids.has(e.from) && ids.has(e.to));
}

/** Open `level` of the default lens through the rail, and wait for its blocks to be drawn. */
async function reach(page: Page, view: MapView, level: string): Promise<void> {
  const label = view.levels?.find((l) => l.id === level)?.label;
  expect(label, `a level labelled ${level}`).toBeDefined();
  await page
    .locator(".rail")
    .getByRole("button", { name: label!, exact: true })
    .click();
  await expect(page.locator(".breadcrumb .lid-name")).toHaveText(label!);
  await page.locator(".map[data-ready]").waitFor();
}

/** Wait until the page draws exactly the blocks of `view`, by their IDs. */
async function drawn(page: Page, view: MapView): Promise<Locator> {
  const ids = view.nodes.map((n) => n.id).sort();
  await expect
    .poll(async () =>
      (
        await page
          .locator(".react-flow__node")
          .evaluateAll((els) => els.map((e) => e.getAttribute("data-id") ?? ""))
      ).sort(),
    )
    .toEqual(ids);
  return page.locator(".block");
}

test.describe("at 1440x900", () => {
  test.use({ viewport: DESKTOP });

  test("the_first_look_draws_every_block_in_its_tone_with_its_kind", async ({
    page,
    engine,
  }) => {
    const top = engineMap(engine);
    const containerLevel = "container";
    const inner = engineMap(engine, { level: containerLevel });
    await openMap(page, await engine.loginUrl());

    const got = new Set<string>();
    for (const [view, open] of [
      [top, () => Promise.resolve()],
      [inner, () => reach(page, top, containerLevel)],
    ] as const) {
      await open();
      const blocks = await drawn(page, view);
      await expect(blocks).toHaveCount(view.nodes.length);
      const nodes = new Map(view.nodes.map((n) => [n.id, n]));
      const looks = await page.locator(".react-flow__node").evaluateAll((els) =>
        els.map((e) => {
          const block = e.querySelector(".block")!;
          return {
            id: e.getAttribute("data-id") ?? "",
            name: block.querySelector(".block-name")?.textContent ?? "",
            tone: block.getAttribute("data-tone"),
            kind: block.querySelector(".block-kind")?.textContent ?? "",
            fill: getComputedStyle(block).backgroundColor,
          };
        }),
      );
      expect(looks).toHaveLength(view.nodes.length);
      for (const look of looks) {
        const node = nodes.get(look.id)!;
        const tone = node.tone ?? "none";
        expect(look.tone, `${look.name}'s tone`).toBe(tone);
        expect(look.kind, `${look.name}'s kind`).toBe(words(node.kind));
        if (tone !== "none") {
          got.add(tone);
          expect(look.fill, `${look.name}'s fill`).toBe(
            await tokenColor(page, `block-${tone}`, "backgroundColor"),
          );
        }
      }
    }
    const engineTones = new Set(
      [...top.nodes, ...inner.nodes].flatMap((n) => (n.tone ? [n.tone] : [])),
    );
    expect(got).toEqual(engineTones);
    const line = `Tones drawn across the top and ${containerLevel} views: ${[...got].sort().join(", ")}; library ${got.has("library") ? "was" : "was not"} among them, infra ${got.has("infra") ? "was" : "was not"}`;
    console.log(line);
    test.info().annotations.push({ type: "tones", description: line });
  });

  test("every_lifted_edge_draws_in_the_lens_yarn_with_its_count", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    const lens = packDefaultLens();
    expect(view.lens, "the default lens").toBe(lens.id);
    expect(view.yarn, "the default lens's yarn").toBe(lens.yarn);
    const edges = onMap(view);
    expect(edges.length).toBeGreaterThan(0);
    await openMap(page, await engine.loginUrl());

    await expect(page.locator(".react-flow__edge")).toHaveCount(edges.length);
    const yarn = await tokenColor(page, `yarn-${view.yarn!}`);
    const strokes = await page
      .locator(".react-flow__edge-path")
      .evaluateAll((els) => els.map((e) => getComputedStyle(e).stroke));
    expect(strokes.length).toBeGreaterThan(0);
    for (const stroke of strokes) {
      expect(stroke).toBe(yarn);
    }
    // Two blocks may each have an edge of one kind to the same target, so count each name.
    const names = new Map(view.nodes.map((n) => [n.id, n.name]));
    const outline = new Map<string, number>();
    for (const e of edges) {
      const name = `${words(e.kind)} → ${names.get(e.to)}, ${e.count}`;
      outline.set(name, (outline.get(name) ?? 0) + 1);
    }
    for (const [name, n] of outline) {
      await expect(
        page.getByRole("treeitem", { name, exact: true }),
      ).toHaveCount(n);
    }

    // The canvas labels an edge only above a count of 1, so look for a view that has one: the
    // default lens at the top, then at each level of the ladder.
    const levels = (view.levels ?? []).filter((l) => !l.apart);
    const candidates: [string, MapView, string | undefined][] = [
      ["the top view", view, undefined],
      ...levels.map((l): [string, MapView, string | undefined] => [
        `the ${l.id} view`,
        engineMap(engine, { level: l.id }),
        l.id,
      ]),
    ];
    const found = candidates.find(([, v]) => onMap(v).some((e) => e.count > 1));
    expect(found, "a view with an edge of count above 1").toBeDefined();
    const [where, labelled, level] = found!;
    if (level !== undefined) {
      await reach(page, view, level);
    }
    const counts = onMap(labelled)
      .filter((e) => e.count > 1)
      .map((e) => String(e.count))
      .sort();
    await expect
      .poll(async () =>
        (await page.locator(".react-flow__edge-text").allTextContents()).sort(),
      )
      .toEqual(counts);
    const line = `Edge count labels checked on ${where}: ${counts.join(", ")}`;
    console.log(line);
    test.info().annotations.push({ type: "labels", description: line });
  });

  test("picking_a_lens_draws_its_edges_in_its_yarn", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    const levels = (view.levels ?? []).filter((l) => !l.apart);
    await openMap(page, await engine.loginUrl());
    const items = lensItems(engine);
    expect(items.length, "the listing's lenses").toBeGreaterThan(1);
    for (const item of items) {
      await page.getByRole("button", { name: CANVAS_STRINGS.lenses }).click();
      await page
        .locator(".picker-sheet")
        .getByRole("button", { name: item.name, exact: true })
        .click();
      // Step down the rail until the lens has an edge to draw.
      let found: { level: string; edges: number; yarn: string } | undefined;
      for (const level of levels) {
        const lensView = engineMap(engine, { lens: item.id, level: level.id });
        const edges = onMap(lensView).length;
        if (edges > 0) {
          found = { level: level.id, edges, yarn: lensView.yarn! };
          break;
        }
      }
      expect(found, `${item.id} has an edge at some level`).toBeDefined();
      const label = view.levels!.find((l) => l.id === found!.level)!.label;
      await page
        .locator(".rail")
        .getByRole("button", { name: label, exact: true })
        .click();
      const yarn = await tokenColor(page, `yarn-${found!.yarn}`);
      const strokes = () =>
        page
          .locator(".react-flow__edge-path")
          .evaluateAll((els) => [
            ...new Set(els.map((e) => getComputedStyle(e).stroke)),
          ]);
      await expect.poll(strokes, `${item.id}'s edges`).toEqual([yarn]);
      await expect(page.locator(".react-flow__edge")).toHaveCount(found!.edges);
    }
  });

  test("a_blocks_counts_drop_whole_levels_from_the_bottom", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    await openMap(page, await engine.loginUrl());
    const blocks = await drawn(page, view);
    await expect(blocks).toHaveCount(view.nodes.length);
    const nodes = new Map(view.nodes.map((n) => [n.id, n]));
    const boxes = await page.locator(".react-flow__node").evaluateAll((els) =>
      els.map((e) => {
        const area = e.querySelector(".block-counts")?.getBoundingClientRect();
        return {
          id: e.getAttribute("data-id") ?? "",
          name: e.querySelector(".block-name")?.textContent ?? "",
          area: area ? { top: area.top, bottom: area.bottom } : null,
          // What each count's `::before` draws; the first has none.
          separators: [...e.querySelectorAll(".block-count")].map((s) =>
            getComputedStyle(s, "::before").content.replace(/^"|"$/g, ""),
          ),
          spans: [...e.querySelectorAll(".block-count")].map((s) => {
            const box = s.getBoundingClientRect();
            return {
              text: s.textContent ?? "",
              top: box.top,
              bottom: box.bottom,
            };
          }),
        };
      }),
    );
    expect(boxes).toHaveLength(view.nodes.length);
    const tolerance = 0.5;
    let withCounts = 0;
    for (const block of boxes) {
      const wanted = countWords(view, nodes.get(block.id)!);
      expect(
        block.spans.map((s) => s.text),
        `${block.name}'s count words`,
      ).toEqual(wanted);
      if (wanted.length === 0) {
        expect(block.area, `${block.name} draws no counts`).toBeNull();
        continue;
      }
      withCounts += 1;
      for (const separator of block.separators.slice(1)) {
        expect(
          separator.startsWith("\u00a0"),
          `${block.name}: a later count's separator starts with a no-break space`,
        ).toBe(true);
      }
      const area = block.area!;
      const visible: string[] = [];
      for (const span of block.spans) {
        const inside =
          span.top >= area.top - tolerance &&
          span.bottom <= area.bottom + tolerance;
        const below = span.top >= area.bottom - tolerance;
        expect(
          inside || below,
          `${block.name}: "${span.text}" is whole inside the counts or wholly below them`,
        ).toBe(true);
        if (inside) {
          visible.push(span.text);
        }
      }
      expect(
        visible.length,
        `${block.name} shows its first count`,
      ).toBeGreaterThan(0);
      expect(visible).toEqual(wanted.slice(0, visible.length));
    }
    expect(withCounts).toBeGreaterThan(0);
  });

  test("a_pinned_block_keeps_its_place_over_the_stored_layout", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    const key = view.layout!;
    const put = page.waitForRequest(
      (r) => r.method() === "PUT" && r.url().endsWith("/api/v1/layout"),
    );
    await openMap(page, await engine.loginUrl());
    await put;
    const base = engine.cli(["map", "--detail", "graph"]).meta.seq;
    const where = () =>
      page
        .locator(".react-flow__node")
        .evaluateAll((els) =>
          Object.fromEntries(
            els.map((e) => [
              e.getAttribute("data-id") ?? "",
              (e as HTMLElement).style.transform,
            ]),
          ),
        );
    const id = Object.keys(await where())[0]!;
    expect(id).toBeTruthy();

    const ok = await page.evaluate(
      async ([node, viewKey, seq]) => {
        const res = await fetch("/api/v1/edit", {
          method: "POST",
          headers: {
            Authorization: `Bearer ${sessionStorage["planroom.token"]}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            op: "pin",
            id: node,
            view: viewKey,
            x: -400,
            y: -300,
            base: seq,
            client: "e2e-pin",
          }),
        });
        return res.ok;
      },
      [id, key.view_key, base] as const,
    );
    expect(ok, "the pin edit").toBe(true);

    // The stream carries the pin to the open page; nothing reloads.
    const pinned = "translate(-400px, -300px)";
    await expect
      .poll(async () => (await where())[id], { timeout: 5000 })
      .toBe(pinned);
    const before = await where();

    await page.reload();
    await page.locator(".map[data-ready]").waitFor();
    await expect.poll(async () => (await where())[id]).toBe(pinned);
    expect(await where()).toEqual(before);
  });
});

test.describe("at 390x844 with touch", () => {
  test.use({ viewport: PHONE, hasTouch: true, isMobile: true });

  test("the_phone_draws_the_same_blocks_and_edges", async ({
    page,
    engine,
  }) => {
    const view = engineMap(engine);
    await openMap(page, await engine.loginUrl());
    const blocks = await drawn(page, view);
    await expect(blocks).toHaveCount(view.nodes.length);
    await expect(page.locator(".react-flow__edge")).toHaveCount(
      onMap(view).length,
    );
  });
});
