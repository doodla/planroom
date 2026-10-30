/**
 * The mod's wiring on a host that lacks a `$` member, which `claude plugin test` can't stand in
 * for: its `$` has every member, and a stand-in hook that throws is skipped, so the mod never sees
 * the TypeError a real host without the member throws. Here `register` runs on a `$` that really
 * lacks it, so the call throws a TypeError at once, as on such a host (DR-129). The hooks run as
 * the host runs them, each handed a `next` that answers for whatever lies beneath the mod.
 */

import type { EngineInterface, Register } from "claude-code";
import { CATALOGUE_HASH, MOD_STRINGS } from "@planroom/contract";
import { fill } from "@planroom/ui";
import { beforeEach, expect, test, vi } from "vitest";

import focusReply from "../../../tests/golden/cli/focus.json";

// The mod's `$.state` atoms, kept in memory: `read` and `update` are the host's module, which only
// a session provides. vitest hoists this above the imports.
const state = vi.hoisted(() => new Map<string, unknown>());
vi.mock("claude-code", () => ({
  atom: (key: { key: string }, initial: unknown) => ({ key: key.key, initial }),
  read: (_$: unknown, a: { key: string; initial: unknown }) =>
    Promise.resolve(state.has(a.key) ? state.get(a.key) : a.initial),
  update: (
    _$: unknown,
    a: { key: string; initial: unknown },
    f: (held: unknown) => unknown,
  ) => {
    const next = f(state.has(a.key) ? state.get(a.key) : a.initial);
    state.set(a.key, next);
    return Promise.resolve(next);
  },
}));

const ROOT = "/plugins/planroom";
const ENGINE = "/repo/target/debug/planroom";
const SESSION = "5e55i0n1-0000-4000-8000-000000000000";
const START = { cwd: "/work", surface: "terminal", isInteractive: true };
const ATTACH = { surface: "terminal", clientId: "terminal:default" };
const CHANGED = fill(MOD_STRINGS.host_changed, { version: "2.1.310" });

type Answer = { exitCode: number; stdout: string; stderr?: string };
type Member = "command.register" | "tool.register" | "ui.open";

/** An engine that resolves, answers focus with the golden PaneModel, and registers the key. */
function readyEngine(argv: readonly string[]): Answer {
  if (argv[0] === `${ROOT}/scripts/planroom` && argv[1] === "version") {
    return {
      exitCode: 0,
      stdout: JSON.stringify({
        schema_version: 1,
        ok: true,
        command: "version",
        result: {
          version: "0.1.0",
          git_commit: "abc1234",
          schema_version: 1,
          catalogue_hash: CATALOGUE_HASH,
        },
        meta: { seq: 0 },
      }),
    };
  }
  if (argv[0] === `${ROOT}/scripts/planroom` && argv[1] === "status") {
    return {
      exitCode: 0,
      stdout: `{"ok":true,"step":"checkout","path":"${ENGINE}","version":"0.1.0"}`,
    };
  }
  if (argv[0] === ENGINE && argv[1] === "focus") {
    return { exitCode: 0, stdout: JSON.stringify(focusReply) };
  }
  if (argv[0] === ENGINE && argv[1] === "person") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"person init","result":{"registered":true},"meta":{"seq":412}}',
    };
  }
  return {
    exitCode: 2,
    stdout: "",
    stderr: `unexpected argv ${argv.join(" ")}`,
  };
}

/** One edit event, a frame that schedules a `focus` refetch. */
const EVENT =
  JSON.stringify({
    type: "event",
    seq: 6,
    kind: "edit",
    target: null,
    project: null,
    actor: { kind: "system", part: "scan" },
    status: "applied",
    summary: "event 6",
  }) + "\n";

/** A `watch` child: it writes its lines, then ends with `ends` or stays alive until it's ended. */
class Child {
  ended = false;
  private at = 0;
  private wake: ((v: IteratorResult<unknown, unknown>) => void) | null = null;

  constructor(
    readonly argv: readonly string[],
    private readonly lines: readonly string[],
    private readonly ends: boolean,
  ) {}

  next(): Promise<IteratorResult<unknown, unknown>> {
    if (this.ended) return Promise.resolve({ done: true, value: undefined });
    const line = this.lines[this.at];
    if (line !== undefined) {
      this.at += 1;
      return Promise.resolve({
        done: false,
        value: { stream: "stdout", text: line },
      });
    }
    if (this.ends) {
      this.ended = true;
      return Promise.resolve({ done: true, value: { code: 4, signal: null } });
    }
    return new Promise((resolve) => {
      this.wake = resolve;
    });
  }

  return(value: unknown): Promise<IteratorResult<unknown, unknown>> {
    this.ended = true;
    this.wake?.({ done: true, value });
    return Promise.resolve({ done: true, value });
  }
}

interface Fake {
  $: EngineInterface;
  runs: string[][];
  children: Child[];
  commands: string[];
  tools: string[];
  statuses: (string | undefined)[];
  toasts: string[];
  /** How many times the mod asked for each member: a call, or for a missing one, a read. */
  asks: Record<string, number>;
}

/** A `$` with every member the mod calls, less `missing`, whose read is counted and is undefined. */
function fakeHost(
  options: {
    missing?: Member;
    /** The missing member is there, but its call rejects with the TypeError instead of throwing it. */
    rejects?: boolean;
    answer?: (argv: readonly string[]) => Answer | Promise<Answer>;
    /** The lines of the nth `watch` child and whether it ends after them; left out, it streams on. */
    watch?: (nth: number) => { lines: string[]; ends: boolean };
  } = {},
): Fake {
  const runs: string[][] = [];
  const children: Child[] = [];
  const commands: string[] = [];
  const tools: string[] = [];
  const statuses: (string | undefined)[] = [];
  const toasts: string[] = [];
  const asks: Record<string, number> = {};
  const ask = (member: string): void => {
    asks[member] = (asks[member] ?? 0) + 1;
  };
  const store = new Map<string, unknown>();
  const answer = options.answer ?? readyEngine;
  const api: Record<string, Record<string, unknown>> = {
    session: {
      version: () => Promise.resolve({ version: "2.1.310", base: "2.1.310" }),
      cwd: () => Promise.resolve(START.cwd),
      id: () => Promise.resolve(SESSION),
      root: () => Promise.resolve(START.cwd),
    },
    clock: {
      now: () => Promise.resolve(1_800_000_000_000),
      // The clock never moves: a sleep never ends, and a timer never fires.
      sleep: () => new Promise(() => undefined),
      after: () => ({ cancel: () => undefined }),
    },
    env: {
      get: (name: string) =>
        Promise.resolve(name === "HOME" ? "/home/u" : undefined),
    },
    store: {
      get: (key: string) => Promise.resolve(store.get(key)),
      set: (key: string, value: unknown) => {
        store.set(key, value);
        return Promise.resolve();
      },
    },
    fs: {
      stat: (path: string) =>
        Promise.resolve({
          kind: "file",
          size: 0,
          mtimeMs: 0,
          isLink: false,
          realPath: path,
        }),
      read: (path: string) => Promise.reject(new Error(`ENOENT: ${path}`)),
    },
    ui: {
      invalidate: () => undefined,
      status: (text: string | undefined) => {
        statuses.push(text);
      },
      toast: (text: string) => {
        toasts.push(text);
      },
      open: () => {
        ask("ui.open");
        return Promise.resolve({ isPlaced: true });
      },
      resolve: () => ({}),
    },
    plugin: { root: ROOT },
    process: {
      run: async (argv: string[]) => {
        runs.push(argv);
        const a = await answer(argv);
        return {
          exitCode: a.exitCode,
          stdout: a.stdout,
          stderr: a.stderr ?? "",
        };
      },
      spawn: ({ argv }: { argv: string[] }) => {
        const script = options.watch?.(children.length) ?? {
          lines: [],
          ends: false,
        };
        const child = new Child(argv, script.lines, script.ends);
        children.push(child);
        return child;
      },
    },
    command: {
      register: (spec: { name: string }) => {
        ask("command.register");
        commands.push(spec.name);
        return Promise.resolve({ command: spec.name });
      },
    },
    tool: {
      register: (spec: { name: string }) => {
        ask("tool.register");
        tools.push(spec.name);
        return Promise.resolve({ tool: `mcp__planroom__${spec.name}` });
      },
    },
  };
  if (options.missing !== undefined) {
    const member = options.missing;
    const [noun = "", name = ""] = member.split(".");
    const group = api[noun] ?? {};
    // A host without the member: reading it gives undefined, so calling it throws a TypeError.
    Object.defineProperty(group, name, {
      enumerable: true,
      get() {
        ask(member);
        return options.rejects === true
          ? () => Promise.reject(new TypeError(`$.${member} is not a function`))
          : undefined;
      },
    });
  }
  return {
    $: api as unknown as EngineInterface,
    runs,
    children,
    commands,
    tools,
    statuses,
    toasts,
    asks,
  };
}

type Hook = (
  $: EngineInterface,
  e: unknown,
  next: (e: unknown) => unknown,
) => unknown;
interface Registered {
  event: string;
  filter: Record<string, unknown> | undefined;
  hook: Hook;
}

/** The mod's hooks, registered through an `on` that records each one. */
async function load(): Promise<Registered[]> {
  vi.resetModules();
  const { register } = await import("./register");
  const hooks: Registered[] = [];
  const on = (event: string, a: unknown, b?: unknown) => {
    const filter = b === undefined ? undefined : (a as Record<string, unknown>);
    hooks.push({ event, filter, hook: (b ?? a) as Hook });
    return { catch: () => undefined };
  };
  register(on as Parameters<Register>[0]);
  return hooks;
}

/** Run the mod's hook for `event` (the one whose filter has `match`), over `beneath`. */
async function raise(
  hooks: Registered[],
  f: Fake,
  event: string,
  e: unknown,
  beneath: unknown,
  match: Record<string, unknown> = {},
): Promise<{ result: unknown; passed: number }> {
  const found = hooks.find(
    (h) =>
      h.event === event &&
      Object.entries(match).every(([k, v]) => h.filter?.[k] === v),
  );
  if (found === undefined) throw new Error(`no ${event} hook`);
  let passed = 0;
  const result = await found.hook(f.$, e, () => {
    passed += 1;
    return Promise.resolve(beneath);
  });
  return { result, passed };
}

/** Let the mod's detached work run: its start-up, the watch loop and the refetches. */
async function settle(): Promise<void> {
  // The fake host answers every call with a settled promise, so its work runs on microtasks.
  for (let i = 0; i < 500; i++) await Promise.resolve();
}

beforeEach(() => {
  state.clear();
});

/**
 * A host lacking `c.lacks`: the session starts as with mods off. `session.start` answers what lies
 * beneath, which ran once; nothing more registers or asks for the pane; the one line shows once;
 * no watch child lives; and every later hook, `/room` and the tools among them, passes through.
 */
async function startsAsWithModsOff(c: {
  lacks: Member;
  rejects?: boolean;
  commands: readonly string[];
  tools: number;
  at: "start" | "attach";
}): Promise<void> {
  const f = fakeHost({
    missing: c.lacks,
    rejects: c.rejects ?? false,
    // A store-backed workspace, so a watch child would live if the loop ran.
    watch: () => ({ lines: [EVENT], ends: false }),
  });
  const hooks = await load();
  const beneath = { cwd: START.cwd };
  const started = await raise(
    hooks,
    f,
    "session.start",
    c.at === "start" ? START : { ...START, surface: null },
    beneath,
  );
  // What lies beneath ran once, and its answer is the hook's.
  expect(started.result).toBe(beneath);
  expect(started.passed).toBe(1);
  await settle();
  if (c.at === "attach") {
    // The start-up ran: the engine resolved and the watch loop started on the store.
    await vi.waitFor(() => expect(f.children).toHaveLength(1));
    const attached = await raise(hooks, f, "session.attach", ATTACH, {
      clientId: ATTACH.clientId,
    });
    expect(attached.passed).toBe(1);
    await settle();
  }

  expect(f.commands).toEqual([...c.commands]);
  expect(f.tools).toHaveLength(c.tools);
  // The pane was asked for once, at the use that found it missing, or never.
  expect(f.asks["ui.open"] ?? 0).toBe(c.lacks === "ui.open" ? 1 : 0);
  if (c.at === "start") expect(f.runs).toEqual([]);
  expect(f.children.filter((child) => !child.ended)).toEqual([]);
  expect(f.statuses.filter((line) => line === CHANGED)).toHaveLength(1);
  expect(f.statuses.at(-1)).toBe(CHANGED);
  const shown = f.statuses.length;
  const runs = f.runs.length;
  const asks = { ...f.asks };

  // Every later hook passes straight through, answering what lies beneath.
  const pass = async (
    event: string,
    e: unknown,
    match: Record<string, unknown> = {},
  ): Promise<void> => {
    const below = { below: event };
    const { result, passed } = await raise(hooks, f, event, e, below, match);
    expect(result, event).toBe(below);
    expect(passed, event).toBe(1);
  };
  await pass("session.attach", ATTACH);
  await pass("ui.render", { props: {} }, { component: "Pane" });
  await pass("ui.render", { props: {} }, { component: "AbovePrompt" });
  await pass("prompt.compose", {});
  await pass("turn.start", { turnId: "t1" });
  await pass("prompt.autocomplete", { text: "/room ", cursor: 6 });
  // The guard doesn't run: the session's own rules decide.
  await pass("tool.check", {
    tool: "Read",
    input: { file_path: ".planroom/store.db" },
  });
  await pass(
    "command.run",
    {
      command: "room",
      args: "map --level system",
      origin: { kind: "composer" },
    },
    { command: "room" },
  );
  await pass(
    "command.run",
    { command: "room", args: "", origin: { kind: "composer" } },
    {
      command: "room",
    },
  );
  await pass(
    "tool.call",
    { tool: "mcp__planroom__room_query", command: "map", args: [] },
    { tool: "mcp__planroom__room_query" },
  );
  await settle();
  expect(f.statuses).toHaveLength(shown);
  expect(f.runs).toHaveLength(runs);
  expect(f.asks).toEqual(asks);
  expect(f.children.filter((child) => !child.ended)).toEqual([]);
}

test("a_host_without_a_registration_member_fails_at_its_first_use: no command.register", async () => {
  await startsAsWithModsOff({
    lacks: "command.register",
    commands: [],
    tools: 0,
    at: "start",
  });
});

test("a_host_without_a_registration_member_fails_at_its_first_use: no tool.register", async () => {
  await startsAsWithModsOff({
    lacks: "tool.register",
    commands: ["room"],
    tools: 0,
    at: "start",
  });
});

test("a_host_without_a_registration_member_fails_at_its_first_use: no ui.open, the pane opened at start", async () => {
  await startsAsWithModsOff({
    lacks: "ui.open",
    commands: ["room"],
    tools: 3,
    at: "start",
  });
});

test("a_host_without_a_registration_member_fails_at_its_first_use: no ui.open, the pane opened at attach", async () => {
  await startsAsWithModsOff({
    lacks: "ui.open",
    commands: ["room"],
    tools: 3,
    at: "attach",
  });
});

test("a_host_without_a_registration_member_fails_at_its_first_use: ui.open rejects with a TypeError at attach", async () => {
  await startsAsWithModsOff({
    lacks: "ui.open",
    rejects: true,
    commands: ["room"],
    tools: 3,
    at: "attach",
  });
});

test("a_failed_host_stays_off_when_work_in_flight_finishes: no watch child starts and its line stays", async () => {
  // The loop's own refetch of focus, and a tool call's run, both held until after the host fails.
  let focuses = 0;
  let releaseFocus: () => void = () => undefined;
  let releaseMap: () => void = () => undefined;
  const focusHeld = new Promise<void>((r) => {
    releaseFocus = r;
  });
  const mapHeld = new Promise<void>((r) => {
    releaseMap = r;
  });
  const f = fakeHost({
    missing: "ui.open",
    answer: async (argv) => {
      if (argv[0] === ENGINE && argv[1] === "focus") {
        focuses += 1;
        if (focuses === 2) await focusHeld;
      }
      if (argv[0] === ENGINE && argv[1] === "map") {
        await mapHeld;
        // A temporary host's warning, whose copy-out line would take the status line.
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            schema_version: 1,
            ok: true,
            command: "map",
            result: { level: "system", nodes: [] },
            meta: {
              seq: 412,
              warnings: [
                {
                  code: "temporary_host",
                  message: "temporary",
                  details: { repo: "/work", ref: "refs/planroom/backup" },
                },
              ],
            },
          }),
        };
      }
      return readyEngine(argv);
    },
    // The first child streams one event, which refetches focus; any later one refuses with a line.
    watch: (nth) =>
      nth === 0
        ? { lines: [EVENT], ends: false }
        : {
            lines: [
              JSON.stringify({
                schema_version: 1,
                ok: false,
                command: "watch",
                error: {
                  code: "schema_mismatch",
                  category: "conflict",
                  retry: "never",
                  message: "A newer engine migrated this store.",
                  hint: "Update Planroom.",
                },
                meta: { seq: 0 },
              }) + "\n",
            ],
            ends: true,
          },
  });
  const hooks = await load();
  await raise(
    hooks,
    f,
    "session.start",
    { ...START, surface: null },
    { cwd: START.cwd },
  );
  // The loop runs, and its refetch is held.
  await vi.waitFor(() => expect(f.children).toHaveLength(1));
  await vi.waitFor(() => expect(focuses).toBe(2));
  // A tool call is mid-run.
  const call = raise(
    hooks,
    f,
    "tool.call",
    {
      tool: "mcp__planroom__room_query",
      command: "map",
      args: ["--level", "system"],
    },
    { result: "beneath" },
    { tool: "mcp__planroom__room_query" },
  );
  await vi.waitFor(() => expect(f.runs.some((r) => r[1] === "map")).toBe(true));

  // A surface attaches, and the pane's open finds the host without ui.open.
  await raise(hooks, f, "session.attach", ATTACH, {
    clientId: ATTACH.clientId,
  });
  await settle();
  expect(f.statuses.at(-1)).toBe(CHANGED);
  const shown = f.statuses.length;

  // The work in flight finishes after the failure.
  releaseFocus();
  releaseMap();
  await call;
  await settle();

  // No watch child starts again, none lives, and the host's line is the last one shown.
  expect(f.children).toHaveLength(1);
  expect(f.children.filter((child) => !child.ended)).toEqual([]);
  expect(f.statuses).toHaveLength(shown);
  expect(f.statuses.at(-1)).toBe(CHANGED);
});
