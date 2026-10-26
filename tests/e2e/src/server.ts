/**
 * A small stand-in for `planroom serve`: it serves the built canvas from `target/canvas/` and
 * answers the API from the golden CLI outputs in `tests/golden/cli/`, with the server's auth
 * rules (DR-29): a one-time login code, a bearer token on every API request, an exact `Origin`,
 * no CORS and a strict CSP. It records what the canvas sends, so specs can check it.
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

import type { Envelope, MapView, Position } from "@planroom/contract";

export const ROOT = fileURLToPath(new URL("../../..", import.meta.url));
export const CANVAS = join(ROOT, "target/canvas");
export const GOLDEN = join(ROOT, "tests/golden/cli");

/** The CSP the engine is to send; React Flow places blocks with inline styles. */
export const CSP =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; " +
  "font-src 'self'; connect-src 'self'; worker-src 'self'; base-uri 'none'; frame-ancestors 'none'";

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
  ".webp": "image/webp",
  ".json": "application/json",
};

export interface LayoutPut {
  view_key: string;
  structure_hash: string;
  positions: Record<string, Position>;
}

export interface Served {
  /** The page's address, with no code. */
  readonly url: string;
  /** Register a one-time login code and return the address that carries it. */
  loginUrl(): string;
  /** Every request's method and path, in order. */
  readonly requests: {
    method: string;
    path: string;
    origin: string | undefined;
  }[];
  readonly layouts: LayoutPut[];
  /** Bodies of commands other than map and focus, by path. */
  readonly runs: { path: string; body: unknown }[];
  /** Forget the stored layout, so the next map is served without positions. */
  resetLayout(): void;
  /** Push one frame to every open stream. */
  push(event: string, data: unknown): void;
  close(): Promise<void>;
}

export interface ServeOptions {
  /** The golden file `map` answers with. */
  map?: string;
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

export async function serve(options: ServeOptions = {}): Promise<Served> {
  const mapEnvelope = JSON.parse(
    await readFile(join(GOLDEN, options.map ?? "map-graph.json"), "utf8"),
  ) as Envelope;
  const focusText = await readFile(join(GOLDEN, "focus.json"), "utf8");
  const codes = new Set<string>();
  const tokens = new Set<string>();
  const streams = new Set<ServerResponse>();
  const sockets = new Set<Socket>();
  let layout: LayoutPut | null = null;
  const requests: Served["requests"] = [];
  const layouts: LayoutPut[] = [];
  const runs: Served["runs"] = [];
  let origin = "";

  const server = createServer((req, res) => {
    void handle(req, res).catch((e: unknown) =>
      json(res, 500, { error: String(e) }),
    );
  });

  async function handle(req: IncomingMessage, res: ServerResponse) {
    const url = new URL(req.url ?? "/", origin);
    const method = req.method ?? "GET";
    requests.push({ method, path: url.pathname, origin: req.headers.origin });
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

    // A same-origin GET carries no Origin header (Fetch: only CORS requests and non-GET ones do),
    // so the stream proves its origin with Sec-Fetch-Site instead.
    const sameOrigin =
      req.headers.origin === origin ||
      (method === "GET" &&
        req.headers.origin === undefined &&
        req.headers["sec-fetch-site"] === "same-origin");
    if (!sameOrigin || req.headers.host !== new URL(origin).host) {
      return json(res, 401, { error: "origin" });
    }

    if (method === "POST" && url.pathname === "/api/v1/session") {
      const { code } = ((await body(req)) ?? {}) as { code?: string };
      if (!code || !codes.delete(code)) {
        return json(res, 401, { error: "code" });
      }
      const token = randomBytes(16).toString("hex");
      tokens.add(token);
      return json(res, 200, { token });
    }

    const auth = req.headers.authorization ?? "";
    if (!auth.startsWith("Bearer ") || !tokens.has(auth.slice(7))) {
      return json(res, 401, { error: "token" });
    }

    if (method === "GET" && url.pathname === "/api/v1/stream") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-store",
      });
      res.write(": open\n\n");
      streams.add(res);
      req.on("close", () => streams.delete(res));
      return;
    }
    if (method === "PUT" && url.pathname === "/api/v1/layout") {
      layout = (await body(req)) as LayoutPut;
      layouts.push(layout);
      res.writeHead(204);
      return res.end();
    }
    if (method === "POST" && url.pathname === "/api/v1/map") {
      const request = ((await body(req)) ?? {}) as {
        level?: MapView["level"] | null;
      };
      const view = structuredClone(mapEnvelope.result) as MapView;
      // The stand-in has one level's blocks; it answers a level asked for with them.
      view.level = request.level ?? view.level;
      if (layout) {
        for (const node of view.nodes) {
          node.position = layout.positions[node.id] ?? null;
        }
      }
      return json(res, 200, { ...mapEnvelope, result: view });
    }
    if (method === "POST" && url.pathname === "/api/v1/focus") {
      await body(req);
      res.writeHead(200, { "Content-Type": "application/json" });
      return res.end(focusText);
    }
    if (method === "POST") {
      runs.push({ path: url.pathname, body: await body(req) });
      return json(res, 200, {
        ...mapEnvelope,
        command: url.pathname.slice(8),
        result: null,
      });
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
    requests,
    layouts,
    runs,
    resetLayout() {
      layout = null;
    },
    push(event, data) {
      for (const s of streams) {
        s.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
      }
    },
    close: () =>
      new Promise<void>((resolve) => {
        streams.forEach((s) => s.end());
        sockets.forEach((s) => s.destroy());
        server.close(() => resolve());
      }),
  };
}
