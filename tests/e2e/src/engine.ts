/**
 * The real engine for the specs: a fresh copy of the built fixture, scanned (or not) by the
 * `planroom` binary `cargo xtask e2e` built with the canvas embedded, and `planroom serve` on it.
 * The specs sign in the way a person does: the first code is the one `serve` wrote into its
 * 0600 login page, and each later one is registered with `serve.json`'s control token, as
 * `planroom open` registers it (DR-29, DR-64).
 */

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
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

export interface Engine {
  /** `http://127.0.0.1:PORT/`, with no code. */
  readonly url: string;
  /** The workspace root, a copy of the fixture. */
  readonly root: string;
  /** A fresh one-time login code, as the address `planroom open` would open. */
  loginUrl(): Promise<string>;
  /** Run one CLI command in the workspace and return its envelope. */
  cli(args: readonly string[]): Envelope;
  /** Write `files` into one of the workspace's repos and commit them there. */
  commit(repo: string, files: Record<string, string>, message: string): void;
  close(): void;
}

export interface EngineOptions {
  /** Scan the workspace before serving it; an unscanned one maps empty. */
  scanned?: boolean;
}

export async function startEngine(
  options: EngineOptions = {},
): Promise<Engine> {
  const tmp = mkdtempSync(join(tmpdir(), "planroom-e2e-"));
  const root = join(tmp, "shop");
  cpSync(FIXTURE, root, { recursive: true, verbatimSymlinks: true });
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PLANROOM_DATA: join(tmp, "data"),
    XDG_CONFIG_HOME: join(tmp, "config"),
    PLANROOM_LOG: "warn",
  };
  delete env.PLANROOM_CLIENT;
  delete env.PLANROOM_ACTOR;

  const cli = (args: readonly string[]): Envelope => {
    const out = spawnSync(BIN, [...args, "--json"], {
      cwd: root,
      env,
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
  const origin = `http://127.0.0.1:${info.port}`;
  const control = (
    JSON.parse(readFileSync(join(root, ".planroom/serve.json"), "utf8")) as {
      control_token: string;
    }
  ).control_token;
  // The first sign-in uses the code serve put in its login page; reading the page is what
  // opening it in a browser does.
  let firstCode: string | null =
    /#code=([0-9a-f]+)/.exec(readFileSync(info.login_page, "utf8"))?.[1] ??
    null;

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
