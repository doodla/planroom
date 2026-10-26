/**
 * The host beneath the mod under `claude plugin test`: the plugin root, the registrations, a clock
 * and a store in memory, and the processes the mod runs, answered from the golden CLI outputs.
 * Each test file imports it; `cargo xtask test` bundles it into each.
 */

import { mock } from "claude-code/testing";
import type { MockClock } from "claude-code/testing";
import type { On, ProcessSpawnChunk, ProcessSpawnResult } from "claude-code";

import focusReply from "../../../tests/golden/cli/focus.json";

export const ROOT = "/plugins/planroom";
export const RESOLVER = `${ROOT}/scripts/planroom`;
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
}

/** An engine that resolves and answers focus with the golden PaneModel. */
export function readyEngine(argv: readonly string[]): Answer {
  if (argv[0] === RESOLVER && argv[1] === "version") {
    return {
      exitCode: 0,
      stdout:
        '{"schema_version":1,"ok":true,"command":"version","result":{"version":"0.1.0","git_commit":"abc1234","schema_version":1,"catalogue_hash":"ab12"},"meta":{"seq":0}}',
    };
  }
  if (argv[0] === RESOLVER && argv[1] === "status") {
    return {
      exitCode: 0,
      stdout: `{"ok":true,"step":"checkout","path":"${ENGINE}","version":"0.1.0"}`,
    };
  }
  if (argv[0] === ENGINE && argv[1] === "focus") {
    return { exitCode: 0, stdout: JSON.stringify(focusReply) };
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

/** Stand in for the host beneath the mod. */
export function host(
  on: On,
  answer: (argv: readonly string[]) => Answer = readyEngine,
  options: {
    store?: Record<string, unknown>;
    /** The script of the nth child `watch` starts; left out, each refuses with `NO_STORE`. */
    watch?: (nth: number) => Piece[];
    /** Symbolic links: where `fs.stat` says each path lands; any other path is itself. */
    links?: Record<string, string>;
  } = {},
): Host {
  const clock = mock.clock(on, { now: 1_800_000_000_000 });
  mock.store(on, options.store ?? {});
  mock.env(on, { CLAUDE_PLUGIN_ROOT: ROOT, HOME: "/home/u" });
  const runs: Run[] = [];
  const children: Child[] = [];
  on("command.register", (_$, e) => ({ value: { command: e.name } }));
  on("tool.register", (_$, e) => ({
    value: { tool: `mcp__planroom__${e.name}` },
  }));
  on("ui.open", () => ({ value: { isPlaced: true } }));
  on("ui.status", () => ({ value: undefined }));
  on("ui.toast", () => ({ value: undefined }));
  on("session.id", () => ({ value: SESSION }));
  on("session.start", (_$, e) => ({ cwd: e.cwd }));
  on("process.run", (_$, e) => {
    runs.push({
      argv: e.argv,
      env: e.init?.env ?? {},
      stdin: e.init?.stdin,
      at: clock.now(),
    });
    const a = answer(e.argv);
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
  on("process.spawn", async function* (_$, e, next) {
    const script = options.watch?.(children.length) ?? [
      { after: 0, text: NO_STORE },
      { after: 0, end: { code: 4, signal: null } },
    ];
    const child = new Child(e.argv, e.env ?? {}, script, clock);
    children.push(child);
    return { value: yield* child.run(next.signal) };
  });
  return { clock, runs, children };
}

/** The focus runs so far, as the clock read when each started. */
export function focusRuns(h: Host): number[] {
  return h.runs.filter((r) => r.argv[1] === "focus").map((r) => r.at);
}
