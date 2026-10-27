/**
 * A token the mod signs, checked by the engine (DR-64, DR-72): with a fixed person key in
 * `$.store` and the host's clock at a fixed instant, the mod's own signing of a typed `/room open`
 * and of its `person init` registration yields exactly these tokens. The engine's verifier takes
 * the same key, line and instant and accepts both in
 * `crates/planroom/tests/actor.rs::a_token_the_mod_signed_verifies_in_the_engine`, so a change on
 * either side that the other doesn't follow fails one of the two. ECDSA here is deterministic
 * (RFC 6979), so the tokens are fixed.
 */

import { expect, test } from "claude-code/testing";

import { isPersonKey, KEY_FORMAT, KEY_STORE } from "../src/actor";
import { ENGINE, host, START } from "./host";

/** The person key whose private scalar is 0x01, then 30 zero bytes, then 0x2a. */
const KEY = {
  format: KEY_FORMAT,
  secret: "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACo",
  public:
    "BEN2rU90TjAtEsnOkmVgumbnM-L-SBpZEHpLvaMXvq100Ge8_e-8IK9n2XODxEPZc2Vd-9VrPveTcKq3WTF7ciU",
  registered: [],
};

/** The host's clock in this test, 1_800_000_000 s: the tokens' `at`. */
const AT = 1_800_000_000;

/** `open` with no arguments, signed at `AT`. */
const OPEN_TOKEN =
  "1800000000.bDwneg_gIbPTaRcJRk2rJGc3ILkYpsLsvTY5DWo63TYxmcKWoh7aLtn3qrSTFsjdqTtTfgQmCej_syhEqc1wiQ";
/** `person init` with no arguments, signed at `AT`. */
const INIT_TOKEN =
  "1800000000.3M57EZA28OLYF87JDWs9o1Db1n9PbHUbwWdmN3Zcm2gAJ5D0nZvcuQ3eaqCqt-dGTVMuun70YOOEbSihAArkbg";

test("a_token_the_mod_signed_is_the_one_the_engine_verifies", async ($, on) => {
  expect(isPersonKey(KEY)).toBe(true);
  const h = host(on, undefined, { store: { [KEY_STORE]: KEY } });
  await $.session.start(START);
  await h.clock.settle();

  const init = h.runs.find(
    (r) => r.argv[1] === "person" && r.argv[2] === "init",
  );
  expect(init?.stdin?.trim()).toBe(KEY.public);
  expect(init?.env["PLANROOM_PERSON_TOKEN"]).toBe(INIT_TOKEN);

  await $.command.run({
    command: "room",
    args: "open",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  const open = h.runs.find((r) => r.argv[1] === "open");
  expect(open?.argv).toEqual([ENGINE, "open", "--json"]);
  expect(open?.env["PLANROOM_ACTOR"]).toBeUndefined();
  expect(open?.env["PLANROOM_PERSON_TOKEN"]).toBe(OPEN_TOKEN);
  expect(Number(OPEN_TOKEN.split(".")[0])).toBe(AT);
});
