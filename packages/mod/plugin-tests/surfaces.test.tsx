/**
 * The mod's drawing under `claude plugin test`, on all four surfaces. `cargo xtask test` bundles
 * this file into target/mod-test/ with the golden CLI outputs inlined.
 */

import { expect, test } from "claude-code/testing";

import {
  BAND,
  BAND_SURFACES,
  DOCK,
  host,
  PANE,
  RESOLVER,
  START,
  SURFACES,
} from "./host";

const NO_RELEASE = JSON.stringify({
  schema_version: 1,
  ok: false,
  command: "version",
  error: {
    code: "engine_unavailable",
    category: "unavailable",
    retry: "never",
    message:
      "Planroom 0.2.0-dev is a development version, so it has no engine release.",
    hint: `${RESOLVER} build`,
    details: {
      reason: "no_release",
      fix: `${RESOLVER} build`,
      est_minutes: 4,
      canvas: true,
    },
  },
  meta: { seq: 0, took_ms: 0, size: { bytes: 400, tokens_est: 100 } },
});

async function texts(ui: {
  findAll: (q: { type: string }) => Promise<{ text: string }[]>;
}): Promise<string[]> {
  return [
    ...(await ui.findAll({ type: "Text" })),
    ...(await ui.findAll({ type: "Button" })),
  ].map((t) => t.text);
}

test("no_blank_pane: before the engine exists the pane offers the source build, on every surface", async ($, on) => {
  const h = host(on, (argv) =>
    argv[1] === "version"
      ? { exitCode: 7, stdout: NO_RELEASE }
      : { exitCode: 2, stdout: "" },
  );
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    expect(
      (await ui.find({ type: "Text", text: /Build the engine from source/ }))
        ?.text,
    ).toBe("Build the engine from source, about 4 minutes?");
    expect((await ui.find({ key: "fallback-action" }))?.text).toContain(
      "Build it",
    );
    for (const t of await texts(ui))
      expect(t.length).toBeLessThanOrEqual(DOCK.columns + 3);
    await ui.unmount();
  }
  for (const surface of BAND_SURFACES) {
    const band = await $.ui.mount({ ...BAND, surface });
    expect((await band.find({ key: "band-primary" }))?.text).toContain(
      "Build it",
    );
    await band.unmount();
  }
  // No engine, so no watch.
  expect(h.children).toHaveLength(0);
});

test("the mod finds its resolver under $.plugin.root, with no CLAUDE_PLUGIN_ROOT in its environment", async ($, on) => {
  // The host doesn't set CLAUDE_PLUGIN_ROOT for a mod (A-14, WS8); the live session found it unset.
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();
  const resolverRuns = h.runs.filter((r) =>
    r.argv[0]?.endsWith("/scripts/planroom"),
  );
  expect(resolverRuns.length).toBeGreaterThan(0);
  // The kit's own `$` has no plugin noun, so check the path's shape: absolute, under the plugin.
  for (const r of resolverRuns) {
    expect(r.argv[0]).toMatch(/^\/.+\/scripts\/planroom$/);
    expect(r.argv[0]).not.toContain("undefined");
  }
});

test("the pane draws the golden focus PaneModel on every surface", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface });
    expect((await ui.find({ type: "Text", text: /^▣ shop/ }))?.text).toBe(
      "▣ shop",
    );
    // The fixture's real pane has no open decision, so the next action is the face-up card.
    expect((await ui.find({ key: "next" }))?.text).toContain(
      "Open the containers",
    );
    expect(
      (await ui.find({ key: "block-worker:repo:worker" }))?.text,
    ).toContain("worker");
    expect(await ui.find({ type: "Text", text: /^No drift/ })).toBeDefined();
    for (const t of await texts(ui))
      expect(t.length).toBeLessThanOrEqual(DOCK.columns + 3);
    await ui.unmount();
  }
  for (const surface of BAND_SURFACES) {
    const band = await $.ui.mount({ ...BAND, surface });
    expect((await band.find({ key: "band-primary" }))?.text).toContain(
      "Open the containers",
    );
    await band.unmount();
  }
});
