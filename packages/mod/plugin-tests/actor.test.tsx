/**
 * The person key and action tokens under `claude plugin test` (DR-64, DR-72): the key is made
 * and registered once, and only the person's presses and typed `/room` lines carry a token, each
 * signed for exactly its own line.
 */

import { expect, test } from "claude-code/testing";

import { KEY_STORE, newPersonKey, verifyToken } from "../src/actor";
import {
  BAND,
  ENGINE,
  host,
  PANE,
  readyEngine,
  START,
  type Answer,
  type Run,
} from "./host";

const MISMATCH = JSON.stringify({
  schema_version: 1,
  ok: false,
  command: "person init",
  error: {
    code: "person_key_mismatch",
    category: "conflict",
    retry: "never",
    message: "This workspace registered another person key.",
  },
  meta: { seq: 412 },
});

function inits(runs: Run[]): Run[] {
  return runs.filter((r) => r.argv[1] === "person" && r.argv[2] === "init");
}

/** The public key the mod registered: what `person init` read on stdin. */
function registered(runs: Run[]): string {
  return (inits(runs)[0]?.stdin ?? "").trim();
}

test("the first start makes a key and registers its public half once per workspace", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();

  const init = inits(h.runs);
  expect(init).toHaveLength(1);
  expect(init[0]?.argv).toEqual([ENGINE, "person", "init", "--json"]);
  const pub = registered(h.runs);
  expect(pub).toMatch(/^[A-Za-z0-9_-]{87}$/);
  // The registration proves it holds the key, and names no agent.
  expect(init[0]?.env["PLANROOM_ACTOR"]).toBeUndefined();
  expect(
    verifyToken(pub, init[0]?.env["PLANROOM_PERSON_TOKEN"] ?? "", {
      command: "person init",
      args: [],
      idem: "",
      base: "",
    }),
  ).toBe(true);

  // A reload keeps the key in $.store and doesn't register it again.
  await $.session.start(START);
  await h.clock.settle();
  expect(inits(h.runs)).toHaveLength(1);
});

test("security-r3-5: a new mod checks its key with the store once, even one an earlier session registered", async ($, on) => {
  // An earlier session registered this key in /work; the store may have lost it since, as after
  // .planroom/ was deleted and rescanned. `person init` with the same key changes nothing.
  const key = { ...newPersonKey(), registered: ["/work"] };
  const h = host(on, readyEngine, { store: { [KEY_STORE]: key } });
  await $.session.start(START);
  await h.clock.settle();
  expect(inits(h.runs)).toHaveLength(1);
  expect(registered(h.runs)).toBe(key.public);
});

test("security-r3-5: a press the store refuses as no person's registers the key again", async ($, on) => {
  let refused = false;
  const h = host(on, (argv): Answer => {
    if (argv[0] === ENGINE && argv[1] === "map" && !refused) {
      refused = true;
      return {
        exitCode: 3,
        stdout: JSON.stringify({
          schema_version: 1,
          ok: false,
          command: "map",
          error: {
            code: "person_required",
            category: "permission",
            retry: "never",
            message: "map needs the person.",
            details: { reason: "bad_signature" },
          },
          meta: { seq: 412 },
        }),
      };
    }
    return readyEngine(argv);
  });
  await $.session.start(START);
  await h.clock.settle();
  expect(inits(h.runs)).toHaveLength(1);

  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  await band.press({ key: "band-primary" });
  await band.unmount();
  await h.clock.settle();
  expect(inits(h.runs)).toHaveLength(2);
});

test("a_press_signs_a_token: the band's press runs as the person, with a token for that line alone", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();
  const pub = registered(h.runs);

  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  await band.press({ key: "band-primary" });
  await band.unmount();

  const map = h.runs.find((r) => r.argv[1] === "map");
  expect(map?.argv).toEqual([ENGINE, "map", "--level", "container", "--json"]);
  expect(map?.env["PLANROOM_ACTOR"]).toBeUndefined();
  const token = map?.env["PLANROOM_PERSON_TOKEN"] ?? "";
  const fields = {
    command: "map",
    args: ["--level", "container"],
    idem: "",
    base: "",
  };
  expect(verifyToken(pub, token, fields)).toBe(true);
  expect(
    verifyToken(pub, token, { ...fields, args: ["--level", "system"] }),
  ).toBe(false);
  // Signed now, in Unix seconds: the engine refuses one more than 30 s off.
  expect(Number(token.split(".")[0])).toBe(Math.floor((map?.at ?? 0) / 1000));
});

test("a /room the person typed carries a token; one a plugin ran doesn't", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();
  const pub = registered(h.runs);
  const presentation = { isFullscreen: true, columns: 120 } as const;

  await $.command.run({
    command: "room",
    args: "map --level system",
    origin: { kind: "composer" },
    presentation,
  });
  const typed = h.runs.filter((r) => r.argv[1] === "map").at(-1);
  expect(
    verifyToken(pub, typed?.env["PLANROOM_PERSON_TOKEN"] ?? "", {
      command: "map",
      args: ["--level", "system"],
      idem: "",
      base: "",
    }),
  ).toBe(true);

  await $.command.run({
    command: "room",
    args: "map --level system",
    origin: { kind: "plugin", name: "another" },
    presentation,
  });
  const ran = h.runs.filter((r) => r.argv[1] === "map").at(-1);
  expect(ran?.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
});

test("a workspace that knows another key shows the rekey pane, and its press re-registers", async ($, on) => {
  let known = false;
  const h = host(on, (argv): Answer => {
    if (argv[1] === "person" && argv[2] === "init") {
      if (!argv.includes("--rekey") && !known)
        return { exitCode: 5, stdout: MISMATCH };
      known = true;
    }
    return readyEngine(argv);
  });
  await $.session.start(START);
  await h.clock.settle();

  const pane = await $.ui.mount({ ...PANE, surface: "terminal" });
  expect((await pane.find({ key: "fallback-action" }))?.text).toContain(
    "Use this session's key",
  );
  // The band never offers it: a stray digit can't replace a key.
  const band = await $.ui.mount({ ...BAND, surface: "terminal" });
  expect(await band.find({ key: "band-primary" })).toBeUndefined();
  await band.unmount();

  await pane.press({ key: "fallback-action" });
  const rekey = inits(h.runs).at(-1);
  expect(rekey?.argv).toEqual([ENGINE, "person", "init", "--rekey", "--json"]);
  expect(
    verifyToken(registered(h.runs), rekey?.env["PLANROOM_PERSON_TOKEN"] ?? "", {
      command: "person init",
      args: ["--rekey"],
      idem: "",
      base: "",
    }),
  ).toBe(true);
  expect(await pane.find({ type: "Text", text: /^▣ shop/ })).toBeDefined();
  await pane.unmount();
});
