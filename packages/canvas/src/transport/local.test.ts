import type { StreamFrame } from "@planroom/contract";
import { describe, expect, test } from "vitest";

import { TOKEN_KEY, localTransport, type LocalEnv } from "./local";
import { CommandError, TransportError } from "./types";

interface Call {
  url: string;
  method: string;
  headers: Record<string, string>;
  body: unknown;
}

type Route = (call: Call) => Response | Promise<Response>;

function memoryStorage(initial: Record<string, string> = {}) {
  const items = new Map(Object.entries(initial));
  return {
    items,
    getItem: (k: string) => items.get(k) ?? null,
    setItem: (k: string, v: string) => void items.set(k, v),
    removeItem: (k: string) => void items.delete(k),
  };
}

function env(routes: Record<string, Route>, over: Partial<LocalEnv> = {}) {
  const calls: Call[] = [];
  const replaced: string[] = [];
  const storage = memoryStorage();
  const fakeFetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
    const call: Call = {
      url,
      method: init?.method ?? "GET",
      headers: { ...(init?.headers as Record<string, string> | undefined) },
      body: typeof init?.body === "string" ? JSON.parse(init.body) : undefined,
    };
    calls.push(call);
    const path = new URL(url).pathname;
    const route = routes[`${call.method} ${path}`];
    if (!route) {
      return new Response("not found", { status: 404 });
    }
    return route(call);
  }) as typeof globalThis.fetch;
  const e: LocalEnv = {
    fetch: fakeFetch,
    storage,
    href: "http://127.0.0.1:41873/#code=one-time",
    replaceUrl: (url) => void replaced.push(url),
    sleep: () => new Promise((resolve) => setTimeout(resolve, 1)),
    ...over,
  };
  return { env: e, calls, replaced, storage };
}

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });

const envelope = (result: unknown, ok = true, command = "map", seq = 1) => ({
  schema_version: 1,
  ok,
  command,
  result: ok ? result : null,
  error: ok
    ? null
    : { code: "not_found", message: "No such block.", retry: "never" },
  meta: { seq, size: { bytes: 0, tokens_est: 0 } },
});

describe("login", () => {
  test("the code in the fragment buys a token, which every request then carries", async () => {
    const t = env({
      "POST /api/v1/session": (c) =>
        json(
          envelope(
            {
              token:
                c.body && (c.body as { code: string }).code === "one-time"
                  ? "tok-1"
                  : "x",
            },
            true,
            "serve",
          ),
        ),
      "POST /api/v1/map": () => json(envelope({ nodes: [] })),
    });
    const transport = localTransport(t.env);
    expect(await transport.login()).toBe(true);
    expect(t.replaced).toEqual(["/"]);
    expect(t.storage.items.get(TOKEN_KEY)).toBe("tok-1");
    expect((await transport.query("map", { detail: "graph" })).result).toEqual({
      nodes: [],
    });
    const map = t.calls.at(-1)!;
    expect(map.headers.Authorization).toBe("Bearer tok-1");
    expect(map.body).toEqual({ detail: "graph" });
  });

  test("signing in again after the code was spent keeps the token and spends nothing", async () => {
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok-1" }, true, "serve")),
    });
    const transport = localTransport(t.env);
    expect(await transport.login()).toBe(true);
    // The page's address was read once, so its fragment still holds the spent code; a retry after
    // the engine stopped answering signs in again with the token the tab already has.
    expect(await transport.login()).toBe(true);
    expect(
      t.calls.filter((c) => c.url.endsWith("/api/v1/session")),
    ).toHaveLength(1);
  });

  test("a reload keeps the tab signed in with the token from sessionStorage", async () => {
    const t = env({}, { href: "http://127.0.0.1:41873/" });
    t.storage.items.set(TOKEN_KEY, "tok-0");
    const transport = localTransport({ ...t.env, storage: t.storage });
    expect(await transport.login()).toBe(true);
    expect(t.calls).toHaveLength(0);
  });

  test("no code and no token means the tab needs a fresh link", async () => {
    const t = env({}, { href: "http://127.0.0.1:41873/" });
    expect(await localTransport(t.env).login()).toBe(false);
  });

  test("a spent code is refused and leaves no token", async () => {
    const t = env({
      "POST /api/v1/session": () =>
        json(
          {
            ...envelope(null, false, "serve"),
            error: { code: "unauthorized", message: "x", retry: "never" },
          },
          401,
        ),
    });
    expect(await localTransport(t.env).login()).toBe(false);
    expect(t.storage.items.has(TOKEN_KEY)).toBe(false);
  });

  test("blocked storage still signs the page in for its lifetime", async () => {
    const throwing = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
      removeItem: () => {
        throw new Error("blocked");
      },
    };
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok-2" }, true, "serve")),
      "POST /api/v1/focus": () => json(envelope({ ok: 1 })),
    });
    const transport = localTransport({ ...t.env, storage: throwing });
    expect(await transport.login()).toBe(true);
    expect((await transport.query("focus", {})).result).toEqual({ ok: 1 });
  });
});

describe("requests", () => {
  async function signedIn(routes: Record<string, Route>) {
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok" }, true, "serve")),
      ...routes,
    });
    const transport = localTransport(t.env);
    await transport.login();
    return { t, transport };
  }

  test("a refused token signs the tab out", async () => {
    const { t, transport } = await signedIn({
      "POST /api/v1/map": () => json({}, 401),
    });
    await expect(transport.query("map", {})).rejects.toMatchObject({
      kind: "needs_login",
    });
    expect(t.storage.items.has(TOKEN_KEY)).toBe(false);
    await expect(transport.query("map", {})).rejects.toBeInstanceOf(
      TransportError,
    );
  });

  test("a failed envelope is a command error carrying the engine's message", async () => {
    const { transport } = await signedIn({
      "POST /api/v1/show": () => json(envelope(null, false), 404),
    });
    const error = await transport.query("show", {}).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(CommandError);
    expect((error as CommandError).message).toBe("No such block.");
  });

  test("an unreachable engine is its own error", async () => {
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok" }, true, "serve")),
    });
    let up = true;
    const fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      if (!up) {
        throw new TypeError("Failed to fetch");
      }
      return t.env.fetch(input, init);
    }) as typeof globalThis.fetch;
    const transport = localTransport({ ...t.env, fetch });
    await transport.login();
    up = false;
    await expect(transport.query("map", {})).rejects.toMatchObject({
      kind: "unreachable",
    });
  });

  test("a command path joins its words with slashes, and a handle runs as its request", async () => {
    const { t, transport } = await signedIn({
      "POST /api/v1/scan": () => json(envelope({ scanned: true })),
    });
    await transport.run({
      command: "scan",
      args: ["/home/you/shop"],
      label: "Map these 5 repos?",
    });
    expect(t.calls.at(-1)).toMatchObject({
      url: "http://127.0.0.1:41873/api/v1/scan",
      body: { dir: "/home/you/shop" },
    });
  });

  test("a_reply_carries_the_envelopes_warnings_and_next", async () => {
    const warning = {
      code: "partial_scan",
      message: "Repo a was skipped.",
      details: null,
    };
    const { transport } = await signedIn({
      "POST /api/v1/ask": () =>
        json({
          ...envelope({ items: [] }, true, "ask"),
          meta: {
            seq: 1,
            size: { bytes: 0, tokens_est: 0 },
            warnings: [warning],
            next: "page-2",
          },
        }),
      "POST /api/v1/focus": () => json(envelope({ ok: 1 }, true, "focus")),
      "POST /api/v1/scan": () =>
        json({
          ...envelope({ scanned: true }, true, "scan"),
          meta: {
            seq: 2,
            size: { bytes: 0, tokens_est: 0 },
            warnings: [warning],
          },
        }),
    });
    expect(await transport.query("ask", {})).toEqual({
      result: { items: [] },
      warnings: [warning],
      next: "page-2",
    });
    // An envelope that carries neither has none.
    expect(await transport.query("focus", {})).toEqual({
      result: { ok: 1 },
      warnings: [],
      next: null,
    });
    // A press carries the warnings of its reply too.
    const press = await transport.run({
      command: "scan",
      args: ["/x"],
      label: "",
    });
    expect(press.warnings).toEqual([warning]);
  });

  test("a layout is stored with PUT", async () => {
    const { t, transport } = await signedIn({
      "PUT /api/v1/layout": () => new Response(null, { status: 204 }),
    });
    const layout = {
      view_key: "map:container:all",
      structure_hash: "ab",
      positions: { a: { x: 1, y: 2 } },
    };
    await transport.saveLayout(layout);
    expect(t.calls.at(-1)).toMatchObject({ method: "PUT", body: layout });
  });
});

describe("the stream", () => {
  /** A stream of `text` that closes, or stays open like a live one. */
  function sse(text: string[], close = true): Response {
    const encoder = new TextEncoder();
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        text.forEach((t) => controller.enqueue(encoder.encode(t)));
        if (close) {
          controller.close();
        }
      },
    });
    return new Response(body, {
      headers: { "Content-Type": "text/event-stream" },
    });
  }

  test("frames arrive with the token in a header, and a reconnect resumes from the last id", async () => {
    let connects = 0;
    const seen: Record<string, string>[] = [];
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok" }, true, "serve")),
      "GET /api/v1/stream": (c) => {
        seen.push(c.headers);
        connects++;
        return connects === 1
          ? sse(['event: event\nid: 5\ndata: {"type":"reset","seq":5}\n', "\n"])
          : sse(['event: ping\ndata: {"type":"ping"}\n\n'], false);
      },
    });
    const transport = localTransport(t.env);
    await transport.login();
    const frames: StreamFrame[] = [];
    const stop = transport.subscribe(
      (f) => frames.push(f),
      () => undefined,
    );
    await expect.poll(() => frames.length).toBe(2);
    stop();
    expect(frames).toEqual([{ type: "reset", seq: 5 }, { type: "ping" }]);
    expect(seen[0]?.Authorization).toBe("Bearer tok");
    expect(seen[1]?.["Last-Event-ID"]).toBe("5");
  });

  test("the stream opens from the oldest seq the loaded views were read at", async () => {
    const seen: Call[] = [];
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok" }, true, "serve")),
      "POST /api/v1/map": () => json(envelope({}, true, "map", 7)),
      "POST /api/v1/focus": () => json(envelope({}, true, "focus", 5)),
      "GET /api/v1/stream": (c) => {
        seen.push(c);
        switch (seen.length) {
          case 1:
            return new Response("down", { status: 503 });
          case 2:
            return sse([
              'id: 9\nevent: event\ndata: {"type":"reset","seq":9}\n\n',
            ]);
          case 3:
            return sse(['event: ping\ndata: {"type":"ping"}\n\n']);
          default:
            return sse(['event: ping\ndata: {"type":"ping"}\n\n'], false);
        }
      },
    });
    const transport = localTransport(t.env);
    await transport.login();
    await Promise.all([
      transport.query("map", {}),
      transport.query("focus", {}),
    ]);
    const stop = transport.subscribe(
      () => undefined,
      () => undefined,
    );
    await expect.poll(() => seen.length).toBeGreaterThanOrEqual(4);
    stop();
    expect(seen[0]?.url.endsWith("/stream?since=5")).toBe(true);
    expect(seen[1]?.url.endsWith("/stream?since=5")).toBe(true);
    expect(seen[0]?.headers["Last-Event-ID"]).toBeUndefined();
    expect(seen[1]?.headers["Last-Event-ID"]).toBeUndefined();
    for (const c of seen.slice(2, 4)) {
      expect(c.url.endsWith("/stream")).toBe(true);
      expect(c.headers["Last-Event-ID"]).toBe("9");
    }
  });

  test("a refused token ends the stream as needs-login", async () => {
    const t = env({
      "POST /api/v1/session": () =>
        json(envelope({ token: "tok" }, true, "serve")),
      "GET /api/v1/stream": () => json({}, 401),
    });
    const transport = localTransport(t.env);
    await transport.login();
    const ended: TransportError[] = [];
    transport.subscribe(
      () => undefined,
      (e) => ended.push(e),
    );
    await expect.poll(() => ended.length).toBe(1);
    expect(ended[0]?.kind).toBe("needs_login");
  });
});
