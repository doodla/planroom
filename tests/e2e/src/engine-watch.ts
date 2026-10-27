/**
 * Vitest global setup: one real `planroom watch --json` run on the built fixture, for
 * `packages/mod/src/watch-engine.test.ts`, which feeds it through the mod's own watch-loop
 * functions. The mod has no Node APIs, so the process work happens here and the test gets only
 * what the child wrote, as `inject("watchRun")`.
 *
 * The run: build the fixture, `scan`, start `watch --json --since <scan seq - 2> --session itest`,
 * commit in two repos, `scan` again, then move the store's `user_version` past this engine's, as a
 * newer engine's migration would, so the child ends with its `schema_mismatch` envelope (DR-78).
 * It needs `target/debug/planroom`, which `cargo xtask test` builds before vitest runs; without
 * it the test is skipped and says why, except under `CI=true`, where it fails.
 */

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** What the run hands the test; the same shape is declared beside the test. */
export interface WatchRun {
  /** Why the run didn't happen, or null when it did. */
  skipped: string | null;
  /** Whether a missing run fails the test rather than skipping it: under `CI=true`. */
  required: boolean;
  /** The `--since` the child started from. */
  since: number;
  /** Each piece of stdout as it arrived, with its time in ms since the child started. */
  chunks: { at: number; text: string }[];
  /** `meta.seq` of each scan: the first, then the rescan after the commits. */
  scans: number[];
  /** The child's exit code. */
  exitCode: number | null;
}

interface Project {
  provide(key: "watchRun", value: WatchRun): void;
}

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const WAIT_MS = 20_000;

const GIT_ENV = {
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_AUTHOR_NAME: "Carol Diaz",
  GIT_AUTHOR_EMAIL: "carol@example.test",
  GIT_AUTHOR_DATE: "2026-09-23T09:00:00Z",
  GIT_COMMITTER_NAME: "Carol Diaz",
  GIT_COMMITTER_EMAIL: "carol@example.test",
  GIT_COMMITTER_DATE: "2026-09-23T09:00:00Z",
};

/** The highest `"seq":N` in `text`: enough to know when to go on, without parsing frames here. */
function maxSeq(text: string): number {
  let max = -1;
  for (const m of text.matchAll(/"seq":(\d+)/g))
    max = Math.max(max, Number(m[1]));
  return max;
}

async function until(what: string, done: () => boolean): Promise<void> {
  const deadline = Date.now() + WAIT_MS;
  while (!done()) {
    if (Date.now() > deadline)
      throw new Error(`engine-watch: timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 20));
  }
}

export default async function setup(project: Project): Promise<() => void> {
  const bin = join(ROOT, "target/debug/planroom");
  if (!existsSync(bin)) {
    project.provide("watchRun", {
      skipped: `${bin} isn't built; cargo xtask test builds it before vitest`,
      required: process.env.CI === "true",
      since: 0,
      chunks: [],
      scans: [],
      exitCode: null,
    });
    return () => {};
  }
  const tmp = mkdtempSync(join(tmpdir(), "planroom-watch-"));
  const shop = join(tmp, "shop");
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PLANROOM_DATA: join(tmp, "data"),
  };
  delete env["PLANROOM_CLIENT"];
  delete env["PLANROOM_ACTOR"];
  delete env["PLANROOM_PERSON_TOKEN"];
  const scan = (): number => {
    const out = execFileSync(bin, ["scan", "--json"], {
      cwd: shop,
      env,
      encoding: "utf8",
    });
    return (JSON.parse(out) as { meta: { seq: number } }).meta.seq;
  };
  const commit = (
    repo: string,
    file: string,
    text: string,
    subject: string,
  ): void => {
    writeFileSync(join(shop, repo, file), text);
    const opts = {
      cwd: join(shop, repo),
      env: { ...env, ...GIT_ENV },
      stdio: "ignore" as const,
    };
    execFileSync("git", ["add", "--", file], opts);
    execFileSync(
      "git",
      ["-c", "commit.gpgsign=false", "commit", "-q", "-m", subject],
      opts,
    );
  };

  execFileSync("python3", [join(ROOT, "examples/build-fixture"), shop], {
    stdio: "ignore",
  });
  const first = scan();
  const since = first - 2;
  const started = performance.now();
  const child = spawn(
    bin,
    ["watch", "--json", "--since", String(since), "--session", "itest"],
    {
      cwd: shop,
      env,
      stdio: ["ignore", "pipe", "inherit"],
    },
  );
  const chunks: { at: number; text: string }[] = [];
  let text = "";
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (piece: string) => {
    chunks.push({ at: Math.round(performance.now() - started), text: piece });
    text += piece;
  });
  let exitCode: number | null | undefined;
  child.on("exit", (code) => {
    exitCode = code;
  });
  const teardown = (): void => {
    if (exitCode === undefined) child.kill("SIGKILL");
    rmSync(tmp, { recursive: true, force: true });
  };

  try {
    await until(`the replay through seq ${first}`, () => maxSeq(text) >= first);
    commit(
      "worker",
      "worker/audit.py",
      'def record(job_id):\n    return {"job": job_id}\n',
      "Add an audit record helper",
    );
    commit(
      "orders",
      "README.md",
      "# orders\n\nThe order service.\n",
      "Trim the README",
    );
    const second = scan();
    await until(
      `the rescan's frames through seq ${second}`,
      () => maxSeq(text) >= second,
    );
    // A newer engine's migration: user_version moves past this engine's in one commit.
    execFileSync("python3", [
      "-c",
      "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); " +
        "v=c.execute('PRAGMA user_version').fetchone()[0]; " +
        "c.execute('PRAGMA user_version = %d' % (v + 1)); c.commit()",
      join(shop, ".planroom/store.db"),
    ]);
    await until("watch to exit on the migration", () => exitCode !== undefined);
    project.provide("watchRun", {
      skipped: null,
      required: process.env.CI === "true",
      since,
      chunks,
      scans: [first, second],
      exitCode: exitCode ?? null,
    });
  } catch (e) {
    teardown();
    throw e;
  }
  return teardown;
}
