/**
 * The real engine for the specs: a fresh copy of the built fixture, scanned (or not) by the
 * `planroom` binary `cargo xtask e2e` built with the canvas embedded, and `planroom serve` on it.
 * The specs sign in the way a person does: the first code is the one `serve` wrote into its
 * 0600 login page, and each later one is registered with the control token in the server's record
 * under the data root, as `planroom open` registers it (DR-29, DR-64, DR-88).
 */

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";

import type { Envelope, ServeInfo } from "@planroom/contract";

import { ROOT } from "./standin";

/** The engine binary, built by `cargo xtask e2e` with `target/canvas/` embedded. */
export const BIN =
  process.env.PLANROOM_E2E_BIN ?? join(ROOT, "target/debug/planroom");
/** The unscanned fixture workspace `cargo xtask e2e` built with `cargo xtask fixture`. */
export const FIXTURE =
  process.env.PLANROOM_E2E_FIXTURE ?? join(ROOT, "target/e2e/fixture/shop");

/** One event as the data root's event backup holds it: the fields a spec checks. */
export interface ExportedEvent {
  seq: number;
  /** The write's idempotency key; a command's events are keyed `KEY#1`, `KEY#2` and so on. */
  idem: string;
  client: { surface: string; id: string };
  [field: string]: unknown;
}

export interface Engine {
  /** `http://127.0.0.1:PORT/`, with no code. */
  readonly url: string;
  /** The workspace root, a copy of the fixture. */
  readonly root: string;
  /** A fresh one-time login code, as the address `planroom open` would open. */
  loginUrl(): Promise<string>;
  /**
   * Run one CLI command in the workspace and return its envelope. `actor` is the `PLANROOM_ACTOR`
   * a write needs (`intent new`); a read sets none.
   */
  cli(args: readonly string[], actor?: string): Envelope;
  /**
   * Every event the person-authored commits wrote through to the data root's event backup
   * (`backups/<workspace_id>/events.ndjson`), oldest first, with the idem and client each carries.
   */
  exported(): ExportedEvent[];
  /**
   * Kill `serve` and start it again on the same workspace, environment and data root. It fails
   * when the new server listens on another port, since a page's `localStorage` is per origin.
   * `loginUrl()` works again after it.
   */
  restart(): Promise<void>;
  /** Write `files` into one of the workspace's repos and commit them there. */
  commit(repo: string, files: Record<string, string>, message: string): void;
  close(): void;
}

export interface EngineOptions {
  /** Scan the workspace before serving it; an unscanned one maps empty. */
  scanned?: boolean;
  /** Changes the workspace copy at `root` before anything scans or serves it. */
  prepare?: (root: string) => void;
}

/** Remove every repo the fixture holds, leaving the workspace directory with none. */
export function removeRepos(root: string): void {
  for (const name of readdirSync(root)) {
    if (existsSync(join(root, name, ".git"))) {
      rmSync(join(root, name), { recursive: true, force: true });
    }
  }
}

/** A new repo `name` in the workspace with no commit, as `git init -b main` leaves it. */
export function initRepo(root: string, name: string): void {
  const out = spawnSync("git", ["init", "--quiet", "-b", "main", name], {
    cwd: root,
    encoding: "utf8",
  });
  if (out.status !== 0) {
    throw new Error(`git init ${name}: ${out.stderr}`);
  }
}

/**
 * A repo git refuses to read: a config it can't parse stands in for any per-repo git failure,
 * as `crates/planroom/tests/scan.rs` makes one.
 */
export function breakRepo(root: string, name: string): void {
  initRepo(root, name);
  writeFileSync(
    join(root, name, ".git/config"),
    "[core\nthis is not a config\n",
  );
}

export async function startEngine(
  options: EngineOptions = {},
): Promise<Engine> {
  const tmp = mkdtempSync(join(tmpdir(), "planroom-e2e-"));
  const root = join(tmp, "shop");
  cpSync(FIXTURE, root, { recursive: true, verbatimSymlinks: true });
  options.prepare?.(root);
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PLANROOM_DATA: join(tmp, "data"),
    XDG_CONFIG_HOME: join(tmp, "config"),
    PLANROOM_LOG: "warn",
  };
  delete env.PLANROOM_CLIENT;
  delete env.PLANROOM_ACTOR;

  const cli = (args: readonly string[], actor?: string): Envelope => {
    const out = spawnSync(BIN, [...args, "--json"], {
      cwd: root,
      env: actor === undefined ? env : { ...env, PLANROOM_ACTOR: actor },
      encoding: "utf8",
    });
    if (out.error) {
      throw new Error(
        `${BIN}: ${out.error.message} (run the specs through cargo xtask e2e)`,
      );
    }
    try {
      return JSON.parse(out.stdout) as Envelope;
    } catch {
      throw new Error(
        `planroom ${args.join(" ")}: exit ${out.status}\n${out.stdout}${out.stderr}`,
      );
    }
  };

  if (options.scanned ?? true) {
    const scan = cli(["scan"]);
    if (!scan.ok) {
      throw new Error(`scan failed: ${JSON.stringify(scan.error)}`);
    }
  }

  /** Start `serve` and wait for its first stdout line, which names the port and the login page. */
  const serve = async (): Promise<{ child: ChildProcess; info: ServeInfo }> => {
    const child: ChildProcess = spawn(BIN, ["serve", "--json"], {
      cwd: root,
      env,
      stdio: ["ignore", "pipe", "inherit"],
    });
    const info = await new Promise<ServeInfo>((resolve, reject) => {
      const lines = createInterface({ input: child.stdout! });
      child.once("error", reject);
      child.once("exit", (code) =>
        reject(new Error(`planroom serve exited with ${code}`)),
      );
      lines.once("line", (line) => {
        const envelope = JSON.parse(line) as Envelope;
        if (!envelope.ok) {
          reject(new Error(`planroom serve: ${line}`));
          return;
        }
        resolve(envelope.result as ServeInfo);
      });
    });
    return { child, info };
  };
  /** The control token is in the server's record under the data root, never in the workspace. */
  const controlOf = (port: number): string =>
    (
      JSON.parse(
        readFileSync(join(tmp, "data", "serve", `${port}.json`), "utf8"),
      ) as {
        control_token: string;
      }
    ).control_token;
  /** The first sign-in uses the code serve put in its login page; reading the page is what opening it in a browser does. */
  const codeOf = (info: ServeInfo): string | null =>
    /#code=([0-9a-f]+)/.exec(readFileSync(info.login_page, "utf8"))?.[1] ??
    null;

  const started = await serve();
  let child = started.child;
  const port = started.info.port;
  const origin = `http://127.0.0.1:${port}`;
  let control = controlOf(port);
  let firstCode = codeOf(started.info);

  return {
    url: `${origin}/`,
    root,
    async loginUrl() {
      if (firstCode !== null) {
        const code = firstCode;
        firstCode = null;
        return `${origin}/#code=${code}`;
      }
      const code = randomBytes(16).toString("hex");
      const response = await fetch(`${origin}/api/v1/code`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${control}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          code_sha256: createHash("sha256").update(code).digest("hex"),
        }),
      });
      if (response.status !== 204) {
        throw new Error(
          `POST /api/v1/code: ${response.status} ${await response.text()}`,
        );
      }
      return `${origin}/#code=${code}`;
    },
    cli,
    exported() {
      const backups = join(tmp, "data", "backups");
      if (!existsSync(backups)) {
        return [];
      }
      return readdirSync(backups)
        .map((id) => join(backups, id, "events.ndjson"))
        .filter((file) => existsSync(file))
        .flatMap((file) => readFileSync(file, "utf8").split("\n"))
        .filter((line) => line.startsWith("{"))
        .map((line) => JSON.parse(line) as Record<string, unknown>)
        .filter((line) => typeof line.idem === "string")
        .map((line) => line as ExportedEvent)
        .sort((a, b) => a.seq - b.seq);
    },
    async restart() {
      const gone = new Promise((resolve) => child.once("exit", resolve));
      child.kill("SIGKILL");
      await gone;
      const next = await serve();
      if (next.info.port !== port) {
        next.child.kill("SIGKILL");
        throw new Error(
          `planroom serve came back on port ${next.info.port}, not ${port}`,
        );
      }
      child = next.child;
      control = controlOf(port);
      firstCode = codeOf(next.info);
    },
    commit(repo, files, message) {
      const dir = join(root, repo);
      for (const [path, text] of Object.entries(files)) {
        mkdirSync(dirname(join(dir, path)), { recursive: true });
        writeFileSync(join(dir, path), text);
      }
      const git = (args: string[]) => {
        const out = spawnSync("git", args, {
          cwd: dir,
          encoding: "utf8",
          env: {
            ...env,
            GIT_AUTHOR_NAME: "E2E",
            GIT_AUTHOR_EMAIL: "e2e@example.test",
            GIT_COMMITTER_NAME: "E2E",
            GIT_COMMITTER_EMAIL: "e2e@example.test",
          },
        });
        if (out.status !== 0) {
          throw new Error(`git ${args.join(" ")}: ${out.stderr}`);
        }
      };
      git(["add", "--all"]);
      git(["commit", "--quiet", "--message", message]);
    },
    close() {
      child.kill("SIGKILL");
      rmSync(tmp, { recursive: true, force: true });
    },
  };
}
