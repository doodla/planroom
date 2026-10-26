/**
 * The Artifact side of `artifact.spec.ts`: a scanned copy of the built fixture, `planroom share`'s
 * bundle of it, and a local host that serves the bundle the way the platform serves a published
 * Artifact: the page fragment wrapped in the platform's skeleton, only the files the publish's
 * `files` map names, by relative path, under a CSP that allows scripts and data only from the
 * page's own files [artifact §1, §5]. `import -` runs the real built engine.
 */

import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";

import type {
  DbBatch,
  Envelope,
  ImportResult,
  ShareResult,
} from "@planroom/contract";

import type { SessionWrite } from "../shims/artifact-db";
import { BIN, FIXTURE } from "./engine";

/** The CSP the host serves the page under: its own files, inline styles, Google Fonts. */
export const CSP = [
  "default-src 'none'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
  "font-src 'self' https://fonts.gstatic.com",
  "img-src 'self' data: blob:",
  "connect-src 'self'",
  "worker-src 'self' blob:",
].join("; ");

const TYPES: Record<string, string> = {
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".webp": "image/webp",
  ".svg": "image/svg+xml",
};

export interface SharedWorkspace {
  readonly root: string;
  readonly share: ShareResult;
  /** Run one CLI command in the workspace, with `stdin`, and return its envelope. */
  cli(args: readonly string[], stdin?: string): Envelope;
  close(): void;
}

/** Copy the fixture, scan it and share it into `.planroom/share`. */
export function shareFixture(): SharedWorkspace {
  const tmp = mkdtempSync(join(tmpdir(), "planroom-artifact-"));
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
  delete env.PLANROOM_PERSON_TOKEN;
  const cli = (args: readonly string[], stdin = ""): Envelope => {
    const out = spawnSync(BIN, [...args, "--json"], {
      cwd: root,
      env,
      input: stdin,
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
  const scan = cli(["scan"]);
  if (!scan.ok) {
    throw new Error(`scan failed: ${JSON.stringify(scan.error)}`);
  }
  const shared = cli(["share", "--out", ".planroom/share"]);
  if (!shared.ok) {
    throw new Error(`share failed: ${JSON.stringify(shared.error)}`);
  }
  return {
    root,
    share: shared.result as ShareResult,
    cli,
    close: () => rmSync(tmp, { recursive: true, force: true }),
  };
}

/** The platform's skeleton around the page fragment [artifact §1]. */
function skeleton(fragment: string): string {
  return (
    '<!doctype html><html><head><meta charset="utf-8">' +
    '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">' +
    "<style>:root{color-scheme:light}body{margin:0}img{max-width:100%}[hidden]{display:none!important}</style>" +
    `</head><body>${fragment}</body></html>`
  );
}

export interface ArtifactHostServer {
  /** The page's address, as a viewer opens the Artifact. */
  readonly url: string;
  /** Every path the page asked for, in order. */
  readonly requested: string[];
  close(): Promise<void>;
}

/** Serve `share`'s publish: the page at `/`, each published file at its own path. */
export async function hostArtifact(
  share: ShareResult,
): Promise<ArtifactHostServer> {
  const page = skeleton(readFileSync(share.publish.file_path, "utf8"));
  const files = share.publish.files;
  const requested: string[] = [];
  const server: Server = createServer((req, res) => {
    const path = decodeURIComponent(
      new URL(req.url ?? "/", "http://host").pathname,
    ).slice(1);
    requested.push(path);
    res.setHeader("Content-Security-Policy", CSP);
    if (path === "") {
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      res.end(page);
      return;
    }
    const source = files[path];
    if (source === undefined) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": TYPES[extname(path)] ?? "application/octet-stream",
    });
    // Each source is relative to the publish's one `root`, as the Artifact tool resolves it.
    res.end(readFileSync(join(share.publish.root, source)));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  return {
    url: `http://127.0.0.1:${port}/`,
    requested,
    close: () => new Promise((resolve) => server.close(() => resolve())),
  };
}

/** A batch with each document read from its batch file, as `ArtifactData` reads `file_path`. */
export function inline(batch: DbBatch): SessionWrite[] {
  return batch.writes.map((w) => ({
    op: w.op,
    collection: w.collection,
    doc_id: w.doc_id,
    ...(w.file_path
      ? {
          data: JSON.parse(readFileSync(w.file_path, "utf8")) as Record<
            string,
            unknown
          >,
        }
      : {}),
    ...(w.if_version !== undefined && w.if_version !== null
      ? { if_version: w.if_version }
      : {}),
  }));
}

/** `import -`'s result, from the engine. */
export function importRows(
  ws: SharedWorkspace,
  lines: readonly object[],
): ImportResult {
  const out = ws.cli(
    ["import", "-"],
    lines.map((l) => JSON.stringify(l)).join("\n") + "\n",
  );
  if (!out.ok) {
    throw new Error(`import failed: ${JSON.stringify(out.error)}`);
  }
  return out.result as ImportResult;
}
