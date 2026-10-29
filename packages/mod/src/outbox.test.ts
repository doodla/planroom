import { expect, test } from "vitest";

import { add, byPerson, clear, entryFor, resend, stale } from "./outbox";
import { agentEnv } from "./tools";

const SESSION = "5e55i0n1-0000-4000-8000-000000000000";
const OTHER = "0th3r5e5-0000-4000-8000-000000000000";

test("reload_mid_write_makes_one_event: the resend is the first run's request, once", () => {
  // The first run: an agent's proposal from a turn, its payload on stdin.
  const argv = [
    "intent",
    "propose",
    "--file",
    "-",
    "--base",
    "41",
    "--idem",
    "toolu_7",
    "--json",
  ];
  const first: Record<string, string> = {
    PLANROOM_DATA: "/home/u/.local/share/planroom",
    ...agentEnv(SESSION, "turn_3"),
  };
  const stdin = '{"stage":"design","items":[]}';
  const entry = entryFor(argv, first, stdin, SESSION);
  expect(entry).not.toBeNull();
  if (entry === null) return;
  // Only names are kept, never a value: no data root, no actor, no token in $.state.
  expect(entry.env_names).toEqual(
    [
      "PLANROOM_ACTOR",
      "PLANROOM_CAUSE",
      "PLANROOM_CLIENT",
      "PLANROOM_DATA",
    ].sort(),
  );
  expect(JSON.stringify(entry)).not.toContain("/home/u/.local/share");
  expect(JSON.stringify(entry)).not.toContain("agent:5e55i0n1");

  let outbox = add([], entry);
  // A reload: the module forgets everything but the atom, and resends what it holds.
  const again = resend(outbox[0]!, SESSION);
  expect(again.argv).toEqual(argv);
  expect(again.stdin).toBe(stdin);
  expect(again.env["PLANROOM_CAUSE"]).toBe("turn:turn_3");
  // The recomputed actor and client equal the first run's for the same session: one request hash.
  expect(again.env["PLANROOM_ACTOR"]).toBe(first.PLANROOM_ACTOR);
  expect(again.env["PLANROOM_CLIENT"]).toBe(first.PLANROOM_CLIENT);
  expect(again.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
  // Its reply clears it, so nothing is resent after.
  outbox = clear(outbox, entry.idem);
  expect(outbox).toEqual([]);
  // Adding the same write twice keeps one entry.
  expect(add(add([], entry), entry)).toHaveLength(1);

  // An entry of another session is dropped, never resent as this one.
  expect(stale(entry, SESSION)).toBe(false);
  expect(stale(entry, OTHER)).toBe(true);
});

test("a person's write is resent with a fresh token for the same line, and a read is never kept", () => {
  const argv = [
    "edit",
    "rename",
    "a",
    "b",
    "--idem",
    "k1",
    "--base",
    "3",
    "--json",
  ];
  const entry = entryFor(
    argv,
    { PLANROOM_CLIENT: `mod:${SESSION}`, PLANROOM_PERSON_TOKEN: "t0ken" },
    undefined,
    SESSION,
  );
  expect(entry).not.toBeNull();
  if (entry === null) return;
  expect(JSON.stringify(entry)).not.toContain("t0ken");
  expect(byPerson(entry)).toBe(true);
  const again = resend(entry, SESSION);
  expect(again.argv).toEqual(argv);
  expect(again.env).toEqual({ PLANROOM_CLIENT: `mod:${SESSION}` });
  expect(again.stdin).toBeUndefined();
  // A line with no --idem is no write the outbox can resend as the same request.
  expect(entryFor(["map", "--json"], {}, undefined, SESSION)).toBeNull();
  expect(
    entryFor(["scan", "--idem=k2", "--json"], {}, undefined, SESSION)?.idem,
  ).toBe("k2");
});
