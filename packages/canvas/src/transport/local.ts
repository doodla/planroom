/**
 * The local transport: `planroom serve` on 127.0.0.1 (architecture.md, "Local server and live
 * stream"; DR-29). The tab trades the one-time code in its URL fragment for a bearer token, keeps
 * it in `sessionStorage`, and sends it on every request and on the stream, which it reads through
 * `fetch` so the token rides in a header. No cookie carries authority.
 */

import type {
  Envelope,
  Handle,
  SessionToken,
  StreamFrame,
} from "@planroom/contract";

import { requestOf } from "./request";
import { SseParser } from "./sse";
import {
  CommandError,
  TransportError,
  type Capabilities,
  type LayoutPut,
  type Reply,
  type Transport,
} from "./types";

/** Where the tab keeps its bearer token. */
export const TOKEN_KEY = "planroom.token";

/** The browser pieces the transport uses, passed in so tests can stand them in. */
export interface LocalEnv {
  readonly fetch: typeof fetch;
  /** The tab's `sessionStorage`, or nothing where storage is blocked. */
  readonly storage: Pick<Storage, "getItem" | "setItem" | "removeItem"> | null;
  /** The page's URL. */
  readonly href: string;
  /** Replace the page's URL without loading it, to drop the spent code from the fragment. */
  readonly replaceUrl: (url: string) => void;
  /** Wait before reconnecting the stream. */
  readonly sleep: (ms: number) => Promise<void>;
}

/** The browser's own pieces. */
export function browserEnv(): LocalEnv {
  // The accessor throws where site data is blocked; the token then lives as long as the page.
  const storage = ((): LocalEnv["storage"] => {
    try {
      return window.sessionStorage;
    } catch {
      return null;
    }
  })();
  return {
    fetch: window.fetch.bind(window),
    storage,
    href: window.location.href,
    replaceUrl: (url) => window.history.replaceState(null, "", url),
    sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  };
}

const RECONNECT_MS = [500, 1000, 2000, 5000];

export function localTransport(env: LocalEnv): Transport {
  const origin = new URL(env.href).origin;
  const api = (path: string) => `${origin}/api/v1/${path}`;

  const read = (): string | null => {
    try {
      return env.storage?.getItem(TOKEN_KEY) ?? null;
    } catch {
      return null;
    }
  };
  const keep = (token: string | null) => {
    try {
      if (token === null) {
        env.storage?.removeItem(TOKEN_KEY);
      } else {
        env.storage?.setItem(TOKEN_KEY, token);
      }
    } catch {
      // Storage blocked: the token lives as long as this page.
    }
  };
  let token = read();
  // The code this page already exchanged. `env.href` is read once, so its fragment keeps naming
  // the spent code; a second `login` (Retry after the engine stopped answering) must not send it.
  let exchanged: string | null = null;
  // The lowest seq any loaded view was read at, until the stream has delivered an id. `map` and
  // `focus` load in parallel, so a commit between their reads must still refetch the older one.
  let floor: number | null = null;
  // The last stream id seen, kept across connections: a connection whose first frame is an id-less
  // ping must not make the next reconnect forget where it was.
  let lastId: string | null = null;

  async function send(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    if (token === null) {
      throw new TransportError("needs_login", "no token");
    }
    let response: Response;
    try {
      response = await env.fetch(api(path), {
        method,
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch (e) {
      throw new TransportError("unreachable", String(e));
    }
    if (response.status === 401) {
      token = null;
      keep(null);
      throw new TransportError("needs_login", "the token was refused");
    }
    return response;
  }

  async function call(name: string, body: unknown): Promise<Envelope> {
    const response = await send("POST", name.split(" ").join("/"), body);
    let envelope: Envelope;
    try {
      envelope = (await response.json()) as Envelope;
    } catch (e) {
      throw new TransportError(
        "unreachable",
        `${response.status}: ${String(e)}`,
      );
    }
    if (!envelope.ok) {
      throw new CommandError(envelope);
    }
    return envelope;
  }

  async function command<T>(name: string, body: unknown): Promise<T> {
    return (await call(name, body)).result as T;
  }

  const replyOf = <T>(envelope: Envelope): Reply<T> => ({
    result: envelope.result as T,
    warnings: envelope.meta?.warnings ?? [],
    next: envelope.meta?.next ?? null,
  });

  const capabilities: Capabilities = { views: "all", layoutWriteBack: true };

  return {
    capabilities,
    partial: null,

    async login() {
      const url = new URL(env.href);
      const code = new URLSearchParams(url.hash.slice(1)).get("code");
      if (code === null || code === exchanged) {
        return token !== null;
      }
      // The code works once: drop it from the address before anything can reuse it.
      url.hash = "";
      env.replaceUrl(url.pathname + url.search);
      let response: Response;
      try {
        response = await env.fetch(api("session"), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ code }),
        });
      } catch (e) {
        throw new TransportError("unreachable", String(e));
      }
      exchanged = code;
      if (!response.ok) {
        token = null;
        keep(null);
        return false;
      }
      // The reply is the envelope every route answers with, its result the token.
      const reply = (await response.json()) as Envelope;
      const issued: unknown = (reply.result as SessionToken | undefined)?.token;
      if (typeof issued !== "string") {
        return false;
      }
      token = issued;
      keep(token);
      return true;
    },

    query: async (name, request) => {
      const envelope = await call(name, request);
      if (lastId === null) {
        const seq = envelope.meta.seq;
        floor = floor === null ? seq : Math.min(floor, seq);
      }
      return replyOf(envelope);
    },
    write: (name, request) => command(name, request),
    run: async (handle: Handle) =>
      replyOf(await call(handle.command, requestOf(handle))),

    async saveLayout(layout: LayoutPut) {
      const response = await send("PUT", "layout", layout);
      if (!response.ok) {
        throw new TransportError("unreachable", `layout: ${response.status}`);
      }
    },

    subscribe(
      onFrame: (frame: StreamFrame) => void,
      onEnd: (error: TransportError) => void,
    ) {
      const abort = new AbortController();
      void (async () => {
        for (let attempt = 0; !abort.signal.aborted; attempt++) {
          try {
            if (token === null) {
              throw new TransportError("needs_login", "no token");
            }
            const headers: Record<string, string> = {
              Authorization: `Bearer ${token}`,
            };
            if (lastId !== null) {
              headers["Last-Event-ID"] = lastId;
            }
            const url =
              lastId === null && floor !== null
                ? api(`stream?since=${floor}`)
                : api("stream");
            const response = await env.fetch(url, {
              headers,
              signal: abort.signal,
            });
            if (response.status === 401) {
              token = null;
              keep(null);
              throw new TransportError("needs_login", "the token was refused");
            }
            if (!response.ok || response.body === null) {
              throw new TransportError(
                "unreachable",
                `stream: ${response.status}`,
              );
            }
            attempt = 0;
            const parser = new SseParser();
            const reader = response.body
              .pipeThrough(new TextDecoderStream())
              .getReader();
            for (;;) {
              const { value, done } = await reader.read();
              if (done) {
                break;
              }
              for (const message of parser.push(value)) {
                lastId = parser.lastEventId ?? lastId;
                try {
                  onFrame(JSON.parse(message.data) as StreamFrame);
                } catch {
                  // A frame that doesn't parse is skipped; the next refetch catches up.
                }
              }
            }
          } catch (e) {
            if (abort.signal.aborted) {
              return;
            }
            if (e instanceof TransportError && e.kind === "needs_login") {
              onEnd(e);
              return;
            }
          }
          const wait =
            RECONNECT_MS[Math.min(attempt, RECONNECT_MS.length - 1)] ?? 5000;
          await env.sleep(wait);
        }
      })();
      return () => abort.abort();
    },
  };
}
