/**
 * The host beneath the mod under `claude plugin test`: the plugin root, the registrations, a clock
 * and a store in memory, and the processes the mod runs, answered from the golden CLI outputs.
 * Each test file imports it; `cargo xtask test` bundles it into each.
 */

import { mock } from "claude-code/testing";
import type { MockClock } from "claude-code/testing";
import type {
  EngineInterface,
  On,
  ProcessSpawnChunk,
  ProcessSpawnResult,
  SessionVersion,
} from "claude-code";

import {
  CATALOGUE_HASH,
  LEAF_OWN_FLAGS,
  type PaneModel,
} from "@planroom/contract";

import focusReply from "../../../tests/golden/cli/focus.json";
import lensPicker from "../../../tests/golden/cli/focus-picker-lens.json";
import projectPicker from "../../../tests/golden/cli/focus-picker-project.json";
import stagePicker from "../../../tests/golden/cli/focus-picker-stage.json";
import { leafOfArgv } from "../src/catalogue";
import { overlay, type Overlay } from "./overlays";

export const ROOT = "/plugins/planroom";
export const RESOLVER = `${ROOT}/scripts/planroom`;

/** Whether argv[0] runs the plugin's resolver, wherever the kit loaded the plugin from (`$.plugin.root`). */
export function isResolver(path: string | undefined): boolean {
  return path !== undefined && path.endsWith("/scripts/planroom");
}
export const ENGINE = "/repo/target/debug/planroom";
export const SESSION = "5e55i0n1-0000-4000-8000-000000000000";
export const SURFACES = ["terminal", "desktop", "vscode", "mobile"] as const;
export const BAND_SURFACES = ["terminal", "desktop"] as const;
export const DOCK = { columns: 49, rows: 34 };

export const PANE = {
  plugin: "planroom",
  component: "Pane",
  requestId: "planroom",
  props: {
    title: "Planroom",
    isFocused: false,
    bodyColumns: DOCK.columns,
    placement: "dock",
    scroll: { offset: 0, bodyRows: DOCK.rows },
    view: {},
  },
} as const;

export const BAND = {
  plugin: "planroom",
  component: "AbovePrompt",
  requestId: "band",
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 10,
    bodyColumns: 75,
    scroll: { offset: 0, bodyRows: 10 },
    view: {},
  },
} as const;

export const START = {
  cwd: "/work",
  surface: "terminal",
  isInteractive: true,
} as const;

export type Answer = { exitCode: number; stdout: string; stderr?: string };

export interface Run {
  argv: readonly string[];
  env: Record<string, string>;
  stdin: string | undefined;
  at: number;
}

/**
 * What `watch` answers when a test doesn't drive it: the refusal an engine gives in a directory
 * with no store, which ends the loop. The kit holds each act while a hook it raised is still
 * running, and a stand-in hook has a 10 s budget, so a test that isn't about the loop keeps none.
 */
export const NO_STORE =
  '{"schema_version":1,"ok":false,"command":"watch","error":{"code":"workspace_not_found","category":"not_found","retry":"never","message":"No Planroom workspace here."},"meta":{"seq":0}}\n';

/** One step of a scripted child: after `after` mocked milliseconds, write `text` or end. */
export type Piece =
  { after: number; text: string } | { after: number; end: ProcessSpawnResult };

/**
 * A child `process.spawn` started, playing its script on the mocked clock: each piece waits its
 * time, then writes or ends. With its script played and no end, it stays alive, as a `watch` does,
 * until the mod ends it. One `clock.advance` plays a whole script, so a test runs in real time
 * well inside a stand-in hook's 10 s budget.
 */
export class Child {
  readonly argv: readonly string[];
  readonly env: Record<string, string>;
  ended: ProcessSpawnResult | null = null;

  constructor(
    argv: readonly string[],
    env: Record<string, string>,
    private readonly script: readonly Piece[],
    private readonly clock: MockClock,
  ) {
    this.argv = argv;
    this.env = env;
  }

  async *run(
    signal: AbortSignal | undefined,
  ): AsyncGenerator<ProcessSpawnChunk, ProcessSpawnResult> {
    const killed = { code: null, signal: "SIGTERM" };
    for (const piece of this.script) {
      if (piece.after > 0) await this.clock.sleep(piece.after);
      if (signal?.aborted === true) return (this.ended = killed);
      if ("end" in piece) return (this.ended = piece.end);
      yield { stream: "stdout", text: piece.text };
    }
    // Alive until the mod ends it; a year on the mocked clock is never.
    await this.clock.sleep(365 * 24 * 3600 * 1000);
    return (this.ended = killed);
  }
}

/** The frames of `text`, written at once, then nothing more. */
export function streams(text: string, after = 0): Piece[] {
  return [{ after, text }];
}

export interface Host {
  clock: MockClock;
  runs: Run[];
  children: Child[];
  /** The slash commands the mod registered, in order. */
  commands: string[];
  /** The tools the mod registered, by name, in order. */
  tools: string[];
  /** The text of each status line the mod set, in order. */
  statuses: (string | undefined)[];
  /** How many times the mod opened its pane. */
  readonly opened: number;
  /** How many times the mod asked for its pane, opened or refused. */
  readonly asked: number;
  /** The toasts the mod showed, in order. */
  toasts: string[];
  /** The shell's directory, as `$.session.cwd()` reports it: a test moves it the way a Bash `cd` does. */
  shell: { cwd: string };
  /** The session's environment, as `$.env.get` reads it now. */
  env: Record<string, string>;
  /**
   * The paths the stand-in treats as deleted, as after a `cargo clean`: `fs.stat` refuses each,
   * and a run of one is recorded and then fails as a spawn of a missing file does. Empty at first.
   */
  gone: Set<string>;
}

/**
 * The flag the engine's parser refuses in a command line after the binary: a `--base` or `--idem`
 * on a leaf whose own flags don't include it. A leaf's own flags are global to its subcommands,
 * so the CLI reads them before or after one (`edit --base SEQ undo EVENT` and `edit undo EVENT
 * --base SEQ` alike). Null for a line it reads. Every engine run here passes this first, as the
 * real parser would, since a mocked answer would otherwise take any argv.
 */
export function unreadFlag(argv: readonly string[]): string | null {
  const l = leafOfArgv(argv);
  if (l === undefined) return null;
  const own = LEAF_OWN_FLAGS[l.name] ?? [];
  for (const word of argv.slice(l.name.split(" ").length)) {
    const flag = word.split("=")[0] ?? word;
    if ((flag === "--base" || flag === "--idem") && !own.includes(flag))
      return flag;
  }
  return null;
}

/** The engine's `invalid_argument` reply to a flag its parser refuses, exit 2. */
function refusedFlag(command: string, flag: string): Answer {
  return {
    exitCode: 2,
    stdout: JSON.stringify({
      schema_version: 1,
      ok: false,
      command,
      error: {
        code: "invalid_argument",
        category: "usage",
        retry: "never",
        message: `unexpected argument '${flag}' found`,
        hint: `planroom schema --command ${command}`,
        details: { field: flag },
      },
      meta: {},
    }),
  };
}

/** The golden picker replies, by the picker's name. */
const PICKERS: Record<string, unknown> = {
  lens: lensPicker,
  project: projectPicker,
  stage: stagePicker,
};

/** An engine that resolves and answers focus with the golden PaneModel, and `focus --picker NAME` with that picker. */
export function readyEngine(argv: readonly string[]): Answer {
  if (isResolver(argv[0]) && argv[1] === "version") {
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
  if (isResolver(argv[0]) && argv[1] === "status") {
    return {
      exitCode: 0,
      stdout: `{"ok":true,"step":"checkout","path":"${ENGINE}","version":"0.1.0"}`,
    };
  }
  if (argv[0] === ENGINE && argv[1] === "focus") {
    const at = argv.indexOf("--picker");
    const picker = at < 0 ? undefined : PICKERS[argv[at + 1] ?? ""];
    return { exitCode: 0, stdout: JSON.stringify(picker ?? focusReply) };
  }
  if (argv[0] === ENGINE && argv[1] === "person" && argv[2] === "init") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"person init","result":{"registered":true},"meta":{"seq":412}}',
    };
  }
  if (argv[0] === ENGINE && argv[1] === "map") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"map","result":{"level":"system","nodes":[]},"meta":{"seq":412,"took_ms":2,"size":{"bytes":9,"tokens_est":3},"truncated":false}}',
    };
  }
  return {
    exitCode: 2,
    stdout: "",
    stderr: `unexpected argv ${argv.join(" ")}`,
  };
}

/**
 * An engine that resolves and answers focus with the golden PaneModel, each overlay laid over it
 * in order, for pane sections the engine can't fill yet (DR-82); every other run as `readyEngine`.
 */
export function overlaid(
  ...overlays: Overlay[]
): (argv: readonly string[]) => Answer {
  const golden = (focusReply as unknown as { result: PaneModel }).result;
  const reply = { ...focusReply, result: overlay(golden, ...overlays) };
  return (argv) =>
    argv[0] === ENGINE && argv[1] === "focus" && !argv.includes("--picker")
      ? { exitCode: 0, stdout: JSON.stringify(reply) }
      : readyEngine(argv);
}

/** Stand in for the host beneath the mod. */
export function host(
  on: On,
  /** How each process run answers; a promise answers when it settles, as a slow run does. */
  answer: (argv: readonly string[]) => Answer | Promise<Answer> = readyEngine,
  options: {
    store?: Record<string, unknown>;
    /** The script of the nth child `watch` starts; left out, each refuses with `NO_STORE`. */
    watch?: (nth: number) => Piece[];
    /** Symbolic links: where `fs.stat` says each path lands; any other path is itself. */
    links?: Record<string, string>;
    /** The session's HOME; null leaves it unset. */
    home?: string | null;
    /** More of the session's environment, beside HOME. */
    env?: Record<string, string>;
    /** Files `fs.read` reads, by absolute path; any other read is refused, as a missing file's is. */
    files?: Record<string, string>;
    /** What the session start beneath the mod does, as another plugin's start would. */
    start?: ($: EngineInterface) => Promise<void>;
    /** The Claude Code version the session reports; left out, the pinned minimum. */
    version?: SessionVersion;
    /**
     * Registrations a hook beneath the mod refuses, each with the reason its deny gives, as the
     * host refuses a command named like a built-in and a `ui.open` hook may refuse a pane. A
     * refused `ui.open` refuses only an open nobody asked for, one without `focus`, so `/room`
     * still opens the pane. A refusal rejects the mod's call with the host's error, never a
     * TypeError, so the mod stays on (DR-129).
     *
     * There is no option for a member the host lacks: the kit's `$` has every member, and a
     * stand-in hook that throws is skipped, so the mod would see the kit's own "no implementation"
     * refusal, never the TypeError a real host without the member throws. `src/register.test.ts`
     * runs the mod on a `$` that lacks one instead.
     */
    refuse?: Partial<
      Record<"command.register" | "tool.register" | "ui.open", string>
    >;
  } = {},
): Host {
  const clock = mock.clock(on, { now: 1_800_000_000_000 });
  mock.store(on, options.store ?? {});
  // No CLAUDE_PLUGIN_ROOT: the host doesn't set it for a mod, which finds itself through $.plugin.root.
  const home = options.home === undefined ? "/home/u" : options.home;
  const env: Record<string, string> = {
    ...(home === null ? {} : { HOME: home }),
    ...options.env,
  };
  // A record the test may change mid-session, as a settings file's `env` changes the host's.
  on("env.get", (_$, e) => ({ value: env[e.name] }));
  const runs: Run[] = [];
  const children: Child[] = [];
  const gone = new Set<string>();
  const shell = { cwd: START.cwd as string };
  on("session.cwd", () => ({ value: shell.cwd }));
  const commands: string[] = [];
  // The host refuses a mod command named like one of its own plugin's skills (DR-84), so the
  // stand-in refuses it too; the mod takes a refused registration as a refusal, not a failed host
  // (DR-129).
  const refuse = options.refuse ?? {};
  on("command.register", (_$, e) => {
    if (refuse["command.register"] !== undefined) {
      return { deny: refuse["command.register"] };
    }
    if (__PLANROOM_SKILLS__.includes(e.name)) {
      return { deny: `a skill of this plugin is already named ${e.name}` };
    }
    commands.push(e.name);
    return { value: { command: e.name } };
  });
  const tools: string[] = [];
  on("tool.register", (_$, e) => {
    if (refuse["tool.register"] !== undefined) {
      return { deny: refuse["tool.register"] };
    }
    tools.push(e.name);
    return { value: { tool: `mcp__planroom__${e.name}` } };
  });
  let opened = 0;
  let asked = 0;
  on("ui.open", (_$, e) => {
    asked += 1;
    if (refuse["ui.open"] !== undefined && e.focus !== true) {
      return { deny: refuse["ui.open"] };
    }
    opened += 1;
    return { value: { isPlaced: true } };
  });
  // The kit's own `$` has no version to ask, so the stand-in answers it whatever the kit would.
  on("session.version", () => ({
    value: options.version ?? {
      version: "2.1.310",
      base: "2.1.310",
      builtAt: "2026-10-23T17:43:32Z",
    },
  }));
  const statuses: (string | undefined)[] = [];
  on("ui.status", (_$, e) => {
    statuses.push(e.text);
    return { value: undefined };
  });
  const toasts: string[] = [];
  on("ui.toast", (_$, e) => {
    toasts.push(e.text);
    return { value: undefined };
  });
  on("session.id", () => ({ value: SESSION }));
  on("session.start", async ($, e) => {
    await options.start?.($);
    return { cwd: e.cwd };
  });
  on("process.run", async (_$, e) => {
    runs.push({
      argv: e.argv,
      env: e.init?.env ?? {},
      stdin: e.init?.stdin,
      at: clock.now(),
    });
    if (e.argv[0] !== undefined && gone.has(e.argv[0])) {
      return { deny: `spawn ${e.argv[0]} ENOENT` };
    }
    const line = e.argv[0] === ENGINE ? e.argv.slice(1) : [];
    const unread = unreadFlag(line);
    const a =
      unread === null
        ? await answer(e.argv)
        : refusedFlag(leafOfArgv(line)?.name ?? "", unread);
    return {
      value: {
        exitCode: a.exitCode,
        stdout: a.stdout,
        stderr: a.stderr ?? "",
        isStdoutTruncated: false,
        isStderrTruncated: false,
      },
    };
  });
  on("fs.stat", (_$, e) => {
    if (gone.has(e.path)) return { deny: `ENOENT: ${e.path}` };
    const realPath = options.links?.[e.path] ?? e.path;
    return {
      value: {
        kind: "file" as const,
        size: 0,
        mtimeMs: 0,
        isLink: realPath !== e.path,
        ...(e.resolve ? { realPath } : {}),
      },
    };
  });
  on("fs.read", (_$, e) => {
    const text = options.files?.[e.path];
    return text === undefined ? { deny: `ENOENT: ${e.path}` } : { value: text };
  });
  on("process.spawn", async function* (_$, e, next) {
    const script = options.watch?.(children.length) ?? [
      { after: 0, text: NO_STORE },
      { after: 0, end: { code: 4, signal: null } },
    ];
    const child = new Child(e.argv, e.env ?? {}, script, clock);
    children.push(child);
    return { value: yield* child.run(next.signal) };
  });
  return {
    clock,
    runs,
    children,
    commands,
    tools,
    statuses,
    gone,
    get opened() {
      return opened;
    },
    get asked() {
      return asked;
    },
    toasts,
    shell,
    env,
  };
}

/** The focus runs so far, as the clock read when each started. */
export function focusRuns(h: Host): number[] {
  return h.runs.filter((r) => r.argv[1] === "focus").map((r) => r.at);
}
