/**
 * A stand-in for `planroom serve`, only for the UI states the engine can't produce yet. It serves
 * the built canvas from `target/canvas/` and answers `map` and `focus` from the real engine's
 * golden outputs in `tests/golden/cli/`, each with at most one test-only overlay from
 * `tests/e2e/overlays/` laid over it. It keeps the real server's auth rules (DR-29) and answers in
 * its envelopes, and it records what the canvas sends, so specs can check it. Every other spec
 * runs against the real engine (`engine.ts`).
 */

import { randomBytes } from "node:crypto";
import { readFile } from "node:fs/promises";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import type { AddressInfo, Socket } from "node:net";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

import type { Envelope, Input, MapView } from "@planroom/contract";

export const ROOT = fileURLToPath(new URL("../../..", import.meta.url));
export const CANVAS = join(ROOT, "target/canvas");
export const GOLDEN = join(ROOT, "tests/golden/cli");
export const OVERLAYS = join(ROOT, "tests/e2e/overlays");

/** The CSP the real server sends (`crates/planroom/src/server/routes.rs`). */
export const CSP =
  "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; " +
  "font-src 'self'; connect-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; " +
  "frame-ancestors 'none'";

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".webp": "image/webp",
  ".json": "application/json",
};

/** A test-only overlay: which golden it lies over, the milestone that makes it real, and why. */
interface Overlay {
  overlay: string;
  milestone: string;
  why: string;
  patch: unknown;
}

export interface StandIn {
  readonly url: string;
  /** Register a one-time login code and return the address that carries it. */
  loginUrl(): string;
  readonly layouts: Input.LayoutPut[];
  /** Bodies of commands other than map and focus, by path. */
  readonly runs: { path: string; body: unknown }[];
  close(): Promise<void>;
}

export interface StandInOptions {
  /** The golden `map` answers with. */
  map?: string;
  /** Overlay files, by name under `tests/e2e/overlays/`. */
  overlays?: readonly string[];
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * `patch` laid over `base`: objects merge key by key, an array of objects with `id`s merges with
 * an object keyed by those ids, and anything else replaces.
 */
export function overlay(base: unknown, patch: unknown): unknown {
  if (Array.isArray(base) && isObject(patch)) {
    return base.map((item: unknown) =>
      isObject(item) && typeof item.id === "string" && item.id in patch
        ? overlay(item, patch[item.id])
        : item,
    );
  }
  if (isObject(base) && isObject(patch)) {
    const out: Record<string, unknown> = { ...base };
    for (const [k, v] of Object.entries(patch)) {
      out[k] = overlay(base[k], v);
    }
    return out;
  }
  return patch;
}

async function golden(
  name: string,
  overlays: readonly Overlay[],
): Promise<Envelope> {
  const envelope = JSON.parse(
    await readFile(join(GOLDEN, name), "utf8"),
  ) as Envelope;
  for (const o of overlays.filter((o) => o.overlay === name)) {
    envelope.result = overlay(envelope.result, o.patch);
  }
  return envelope;
}

async function body(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(chunk as Buffer);
  }
  const text = Buffer.concat(chunks).toString("utf8");
  return text === "" ? undefined : JSON.parse(text);
}

function json(res: ServerResponse, status: number, value: unknown) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(value));
}

const envelope = (command: string, result: unknown) => ({
  schema_version: 1,
  ok: true,
  command,
  result,
  meta: { seq: 0, size: { bytes: 0, tokens_est: 0 } },
});

const unauthorized = (res: ServerResponse) =>
  json(res, 401, {
    schema_version: 1,
    ok: false,
    command: "serve",
    error: {
      code: "unauthorized",
      message: "This tab needs a fresh link: press e in the pane.",
      retry: "never",
    },
    meta: { seq: 0, size: { bytes: 0, tokens_est: 0 } },
  });

export async function standIn(options: StandInOptions = {}): Promise<StandIn> {
  const overlays = await Promise.all(
    (options.overlays ?? []).map(
      async (name) =>
        JSON.parse(await readFile(join(OVERLAYS, name), "utf8")) as Overlay,
    ),
  );
  const mapEnvelope = await golden(options.map ?? "map-graph.json", overlays);
  const focusEnvelope = await golden("focus.json", overlays);
  const codes = new Set<string>();
  const tokens = new Set<string>();
  const streams = new Set<ServerResponse>();
  const sockets = new Set<Socket>();
  let layout: Input.LayoutPut | null = null;
  const layouts: Input.LayoutPut[] = [];
  const runs: StandIn["runs"] = [];
  let origin = "";

  const server = createServer((req, res) => {
    void handle(req, res).catch((e: unknown) =>
      json(res, 500, { error: String(e) }),
    );
  });

  async function handle(req: IncomingMessage, res: ServerResponse) {
    const url = new URL(req.url ?? "/", origin);
    const method = req.method ?? "GET";
    res.setHeader("Content-Security-Policy", CSP);
    res.setHeader("X-Content-Type-Options", "nosniff");

    if (!url.pathname.startsWith("/api/")) {
      if (method !== "GET") {
        return json(res, 405, {});
      }
      const path =
        url.pathname === "/" ? "index.html" : normalize(url.pathname.slice(1));
      if (path.startsWith("..")) {
        return json(res, 404, {});
      }
      try {
        const file = await readFile(join(CANVAS, path));
        res.writeHead(200, {
          "Content-Type": TYPES[extname(path)] ?? "application/octet-stream",
        });
        return res.end(file);
      } catch {
        return json(res, 404, {});
      }
    }

    // The real server's rule: an exact Origin, or Sec-Fetch-Site: same-origin on a GET, which a
    // browser sends without one.
    const sameOrigin =
      req.headers.origin === origin ||
      (method === "GET" &&
        req.headers.origin === undefined &&
        req.headers["sec-fetch-site"] === "same-origin");
    if (!sameOrigin || req.headers.host !== new URL(origin).host) {
      return unauthorized(res);
    }

    if (method === "POST" && url.pathname === "/api/v1/session") {
      const { code } = ((await body(req)) ?? {}) as { code?: string };
      if (!code || !codes.delete(code)) {
        return unauthorized(res);
      }
      const token = randomBytes(32).toString("hex");
      tokens.add(token);
      return json(res, 200, envelope("serve", { token }));
    }

    const auth = req.headers.authorization ?? "";
    if (!auth.startsWith("Bearer ") || !tokens.has(auth.slice(7))) {
      return unauthorized(res);
    }

    if (method === "GET" && url.pathname === "/api/v1/stream") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-store",
      });
      res.write('event: ping\ndata: {"type":"ping"}\n\n');
      streams.add(res);
      req.on("close", () => streams.delete(res));
      return;
    }
    if (method === "PUT" && url.pathname === "/api/v1/layout") {
      layout = (await body(req)) as Input.LayoutPut;
      layouts.push(layout);
      res.writeHead(204);
      return res.end();
    }
    if (method === "POST" && url.pathname === "/api/v1/map") {
      await body(req);
      const view = structuredClone(mapEnvelope.result) as MapView;
      for (const node of view.nodes) {
        node.position = layout?.positions[node.id] ?? null;
      }
      return json(res, 200, { ...mapEnvelope, result: view });
    }
    if (method === "POST" && url.pathname === "/api/v1/focus") {
      await body(req);
      return json(res, 200, focusEnvelope);
    }
    if (method === "POST") {
      runs.push({ path: url.pathname, body: await body(req) });
      return json(res, 200, envelope(url.pathname.slice(8), null));
    }
    return json(res, 404, {});
  }

  server.on("connection", (s) => {
    sockets.add(s);
    s.on("close", () => sockets.delete(s));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  origin = `http://127.0.0.1:${port}`;

  return {
    url: `${origin}/`,
    loginUrl() {
      const code = randomBytes(16).toString("hex");
      codes.add(code);
      return `${origin}/#code=${code}`;
    },
    layouts,
    runs,
    close: () =>
      new Promise<void>((resolve) => {
        streams.forEach((s) => s.end());
        sockets.forEach((s) => s.destroy());
        server.close(() => resolve());
      }),
  };
}
