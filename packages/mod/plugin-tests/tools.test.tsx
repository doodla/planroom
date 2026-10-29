/**
 * The mod's tools, `/room` and the guard under `claude plugin test`: the engine answers from the
 * golden CLI outputs, and each test reads the argv, stdin and environment the mod ran it with.
 */

import { expect, test } from "claude-code/testing";
import { MOD_COMMANDS } from "@planroom/contract";

import {
  ENGINE,
  host,
  readyEngine,
  SESSION,
  START,
  unreadFlag,
  type Answer,
} from "./host";

const AGENT = "agent:5e55i0n1:chat";

const NOT_FOUND = JSON.stringify({
  schema_version: 1,
  ok: false,
  command: "map",
  error: {
    code: "not_found",
    category: "not_found",
    retry: "never",
    message: "No node matches worker2.",
    hint: "planroom find worker2",
    details: { candidates: ["worker:repo:worker"] },
  },
  meta: { seq: 412, took_ms: 1, size: { bytes: 300, tokens_est: 75 } },
});

const SCANNED = JSON.stringify({
  schema_version: 1,
  ok: true,
  command: "scan",
  result: { map: { level: "system", nodes: [] } },
  meta: { seq: 430, took_ms: 900, size: { bytes: 90, tokens_est: 23 } },
});

test("room_query runs the engine as the chat agent and returns one JSON string", async ($, on) => {
  const h = host(on);
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--level", "system"],
  });
  expect(typeof called.result).toBe("string");
  expect(JSON.parse(called.result as string)).toEqual({
    ok: true,
    untrusted: true,
    result: { level: "system", nodes: [] },
    meta: { seq: 412 },
  });
  const map = h.runs.find((r) => r.argv[1] === "map");
  expect(map?.argv).toEqual([ENGINE, "map", "--level", "system", "--json"]);
  expect(map?.env["PLANROOM_ACTOR"]).toBe(AGENT);
  expect(map?.env["PLANROOM_CLIENT"]).toBe(`mod:${SESSION}`);
  expect(map?.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
});

test("error_envelopes_reach_the_model: the whole error object, in the one JSON string", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[1] === "map" ? { exitCode: 3, stdout: NOT_FOUND } : readyEngine(argv),
  );
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--focus", "worker2"],
  });
  expect(JSON.parse(called.result as string)).toEqual({
    ok: false,
    untrusted: true,
    error: {
      code: "not_found",
      category: "not_found",
      retry: "never",
      message: "No node matches worker2.",
      hint: "planroom find worker2",
      details: { candidates: ["worker:repo:worker"] },
    },
    meta: { seq: 412 },
  });
});

test("a_tool_call_carries_the_agent_actor: room_change writes as the agent, payload on stdin", async ($, on) => {
  const h = host(on, (argv): Answer =>
    argv[1] === "scan" ? { exitCode: 0, stdout: SCANNED } : readyEngine(argv),
  );
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const called = await $.tool.call({
    tool: "mcp__planroom__room_change",
    command: "scan",
    args: ["."],
    payload: { note: "inline" },
  });
  expect(JSON.parse(called.result as string)).toMatchObject({
    ok: true,
    meta: { seq: 430 },
  });
  const scan = h.runs.find((r) => r.argv[1] === "scan");
  expect(scan?.env["PLANROOM_ACTOR"]).toBe(AGENT);
  expect(scan?.env["PLANROOM_PERSON_TOKEN"]).toBeUndefined();
  // A write's idem is the call's own id, so a retried call replays (A-8).
  const idem = scan?.argv[scan.argv.indexOf("--idem") + 1];
  expect(typeof idem === "string" && idem.length > 0).toBe(true);
  expect(scan?.argv.slice(-3)).toEqual(["--file", "-", "--json"]);
  expect(scan?.stdin).toBe('{"note":"inline"}');
  // The change redraws the pane.
  expect(h.runs.filter((r) => r.argv[1] === "focus").length).toBe(2);
});

/**
 * The plugin's skills, spelled out: a plugin the kit loads beside the mod runs on its own, so its
 * hook can't read this file's names. The test checks they are the bundle's own.
 */
const SKILLS = ["planroom", "room-handoff", "room-plan"];

test(
  "a_command_named_like_a_skill_is_refused: as the host refuses it, which would skip session.start",
  {
    plugins: [
      {
        // A plugin that registers a command named like each skill at its start, as a mod could
        // by mistake, and toasts each refusal it gets.
        name: "named-like-a-skill",
        register(on) {
          on("session.start", async ($, e, next) => {
            for (const skill of ["planroom", "room-handoff", "room-plan"]) {
              await $.command
                .register({ name: skill, description: "x" })
                .catch((err: unknown) => $.ui.toast(`refused: ${String(err)}`));
            }
            return next(e);
          });
        },
      },
    ],
  },
  async ($, on) => {
    expect([...__PLANROOM_SKILLS__].sort()).toEqual(SKILLS);
    const h = host(on);
    await $.session.start(START);
    await h.clock.settle();
    const refused = h.toasts.filter((t) => t.startsWith("refused: "));
    expect(refused).toHaveLength(SKILLS.length);
    SKILLS.forEach((skill, i) => expect(refused[i]).toContain(skill));
    expect(h.commands.filter((c) => SKILLS.includes(c))).toEqual([]);
    // The mod's own start still ran: its one command is registered.
    expect(h.commands).toEqual([...MOD_COMMANDS]);
  },
);

test("every_command_the_mod_registers_is_in_mod_commands, and none is named like a skill", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();
  expect(h.commands).toEqual([...MOD_COMMANDS]);
  for (const name of h.commands) {
    expect(__PLANROOM_SKILLS__).not.toContain(name);
  }
});

test("a tool's wrong argument returns the leaf's synopsis, and the tool never sends --client", async ($, on) => {
  const h = host(on);
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();
  const before = h.runs.length;
  const called = await $.tool.call({
    tool: "mcp__planroom__room_query",
    command: "map",
    args: ["--client=elsewhere"],
  });
  const result = JSON.parse(called.result as string) as {
    ok: boolean;
    error: { code: string; hint: string; details: { synopsis: string } };
  };
  expect(result.ok).toBe(false);
  expect(result.error.code).toBe("invalid_argument");
  expect(result.error.hint).toBe("planroom schema --command map");
  expect(result.error.details.synopsis).toContain("map [--level");
  // Refused before any process runs.
  expect(h.runs.slice(before).some((r) => r.argv[1] === "map")).toBe(false);
});

test("/room share hands the model the publish's arguments as context", async ($, on) => {
  const shared = JSON.stringify({
    schema_version: 1,
    ok: true,
    command: "share",
    summary: "Ready to publish shop's map.",
    result: {
      summary: "map",
      publish: {
        file_path: "/work/.git/planroom/share/page.html",
        root: "/work/.git/planroom/share",
        files: {},
        capabilities: {},
        title: "shop",
        description: "",
        icon: "map",
      },
      db: [],
      files: [],
      holds: [],
    },
    meta: { seq: 412 },
  });
  const h = host(on, (argv): Answer =>
    argv[1] === "share" ? { exitCode: 0, stdout: shared } : readyEngine(argv),
  );
  await $.session.start(START);
  await h.clock.settle();
  const ran = await $.command.run({
    command: "room",
    args: "share",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  expect(ran.text).toBe("Ready to publish shop's map.");
  expect(ran.context).toHaveLength(1);
  expect(ran.context?.[0]).toContain(
    '"file_path":"/work/.git/planroom/share/page.html"',
  );
  // No --out: the engine writes to the state directory's share/, which the mod never guesses.
  const share = h.runs.find((r) => r.argv[1] === "share");
  expect(share?.argv.some((a) => a.startsWith("--out"))).toBe(false);
});

test("/room help lists the commands; bare /room focuses the pane and returns no text", async ($, on) => {
  const h = host(on);
  await $.session.start(START);
  await h.clock.settle();

  const typed = {
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  } as const;
  const help = await $.command.run({ command: "room", args: "help", ...typed });
  expect(help.text).toContain("/room map");
  const opened = await $.command.run({ command: "room", args: "", ...typed });
  expect(opened.text).toBeUndefined();
  const ran = await $.command.run({
    command: "room",
    args: "map --level system",
    ...typed,
  });
  // The summary line the model reads, never the raw envelope (ux-2).
  expect(ran.text).toBe("No blocks at the system level");
  expect(h.runs.find((r) => r.argv[1] === "map")?.argv).toEqual([
    ENGINE,
    "map",
    "--level",
    "system",
    "--json",
  ]);
  const unknown = await $.command.run({
    command: "room",
    args: "fly",
    ...typed,
  });
  expect(unknown.text).toContain('no command "fly"');
});

/** An engine that answers `edit` and `share` as the real one does, once its parser takes the line. */
function writingEngine(argv: readonly string[]): Answer {
  const ok = (command: string, result: unknown): Answer => ({
    exitCode: 0,
    stdout: JSON.stringify({
      schema_version: 1,
      ok: true,
      command,
      result,
      meta: { seq: 413 },
    }),
  });
  if (argv[1] === "edit") return ok("edit", { summary: "Renamed." });
  if (argv[1] === "share") return ok("share", { summary: "Ready." });
  return readyEngine(argv);
}

test("every write path gives the engine a line its parser reads: --base and --idem before edit's subcommand, none to share", async ($, on) => {
  const h = host(on, writingEngine);
  await $.session.start({ cwd: "/work", surface: null, isInteractive: false });
  await h.clock.settle();

  const edit = await $.tool.call({
    tool: "mcp__planroom__room_change",
    command: "edit",
    args: ["rename", "worker:repo:worker", "Workers"],
    tool_use_id: "toolu_1",
  });
  expect(JSON.parse(edit.result as string)).toMatchObject({ ok: true });
  const ran = h.runs.find((r) => r.argv[1] === "edit")?.argv ?? [];
  // The base is the pane's seq, since no read this turn returned one.
  expect(ran[3]).toMatch(/^\d+$/);
  expect(ran).toEqual([
    ENGINE,
    "edit",
    "--base",
    ran[3],
    "--idem",
    "toolu_1",
    "rename",
    "worker:repo:worker",
    "Workers",
    "--json",
  ]);

  // `share` takes no --idem: the hand-off tool and both /room paths run it bare.
  const shared = await $.tool.call({
    tool: "mcp__planroom__room_handoff",
    command: "share",
    args: [],
    tool_use_id: "toolu_2",
  });
  expect(JSON.parse(shared.result as string)).toMatchObject({ ok: true });
  for (const origin of [
    { kind: "composer" },
    { kind: "plugin", name: "another" },
  ] as const) {
    await $.command.run({
      command: "room",
      args: "share",
      origin,
      presentation: { isFullscreen: true, columns: 120 },
    });
  }
  const shares = h.runs.filter((r) => r.argv[1] === "share");
  expect(shares.map((r) => r.argv)).toEqual([
    [ENGINE, "share", "--json"],
    [ENGINE, "share", "--json"],
    [ENGINE, "share", "--json"],
  ]);

  // A typed /room edit runs as the person, its flags before the subcommand too.
  await $.command.run({
    command: "room",
    args: "edit rename worker:repo:worker Workers",
    origin: { kind: "composer" },
    presentation: { isFullscreen: true, columns: 120 },
  });
  const typed = h.runs.filter((r) => r.argv[1] === "edit").at(-1)?.argv ?? [];
  expect(typed.slice(0, 2)).toEqual([ENGINE, "edit"]);
  expect(typed.slice(-4)).toEqual([
    "rename",
    "worker:repo:worker",
    "Workers",
    "--json",
  ]);
  expect(typed.filter((a) => a === "--idem" || a === "--base")).toHaveLength(2);
  // No run in this test met the parser's refusal.
  expect(h.runs.every((r) => unreadFlag(r.argv.slice(1)) === null)).toBe(true);
});
