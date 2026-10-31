import type { Envelope } from "@planroom/contract";
import { describe, expect, test } from "vitest";

import { flush } from "./flush";
import { pendingStore, type PendingEdit } from "./pending";
import {
  CommandError,
  TransportError,
  type Reply,
  type Transport,
} from "../transport/types";

function memory() {
  const items = new Map<string, string>();
  return {
    getItem: (k: string) => items.get(k) ?? null,
    setItem: (k: string, v: string) => void items.set(k, v),
    removeItem: (k: string) => void items.delete(k),
  };
}

const entry = (idem: string, at: number): PendingEdit => ({
  idem,
  client: "tab:0123456789abcdef",
  request: { op: "hide", id: idem, base: 3, client: "tab:ffffffffffffffff" },
  line: `Hide ${idem}`,
  at,
});

const reply = (replayed: boolean): Reply<unknown> => ({
  result: { event: "e1", count: 1 },
  warnings: [],
  next: null,
  seq: 5,
  replayed,
  summary: "Hid it.",
});

const failure = (
  code: string,
  message: string,
  hint?: string,
  retry = "never",
) =>
  new CommandError({
    schema_version: 1,
    ok: false,
    command: "edit",
    error: { code, message, retry, ...(hint ? { hint } : {}) },
    meta: { seq: 5, size: { bytes: 0, tokens_est: 0 } },
  } as unknown as Envelope);

interface Sent {
  command: string;
  request: Record<string, unknown>;
  idem: string;
}

function fakeTransport(
  answer: (sent: Sent, n: number) => Promise<Reply<unknown>>,
) {
  const sent: Sent[] = [];
  const transport = {
    write: (command: string, request: object, opts: { idem: string }) => {
      const s = {
        command,
        request: request as Record<string, unknown>,
        idem: opts.idem,
      };
      sent.push(s);
      return answer(s, sent.length);
    },
  } as unknown as Transport;
  return { sent, transport };
}

describe("flushing pending edits", () => {
  test("a_replayed_flush_drops_the_edit_as_landed", async () => {
    const mem = memory();
    const store = pendingStore(mem, "w1");
    store.add(entry("u1", 1));
    const { sent, transport } = fakeTransport(() =>
      Promise.resolve(reply(true)),
    );
    const flushed = await flush(transport, store);
    expect(flushed.outcomes).toEqual([
      {
        idem: "u1",
        state: "landed",
        result: { event: "e1", count: 1 },
        summary: "Hid it.",
        replayed: true,
      },
    ]);
    expect(flushed.stopped).toBeNull();
    expect(store.list()).toEqual([]);
    // The entry's own tab wrote it, not whatever the request carried.
    expect(sent[0]?.request["client"]).toBe("tab:0123456789abcdef");
    expect(sent[0]?.idem).toBe("u1");
  });

  test("a_stale_write_drops_the_edit_and_shows_the_conflict", async () => {
    const store = pendingStore(memory(), "w1");
    store.add(entry("u1", 1));
    const { transport } = fakeTransport(() =>
      Promise.reject(failure("stale_write", "M")),
    );
    const flushed = await flush(transport, store);
    expect(flushed.outcomes).toEqual([
      { idem: "u1", state: "conflict", message: "M" },
    ]);
    expect(store.list()).toEqual([]);
  });

  test("any_other_refusal_drops_the_edit_with_its_hint", async () => {
    const store = pendingStore(memory(), "w1");
    store.add(entry("u1", 1));
    const { transport } = fakeTransport(() =>
      Promise.reject(failure("not_found", "No such block.", "planroom find x")),
    );
    const flushed = await flush(transport, store);
    expect(flushed.outcomes).toEqual([
      {
        idem: "u1",
        state: "refused",
        message: "No such block.",
        hint: "planroom find x",
      },
    ]);
    expect(store.list()).toEqual([]);
  });

  test("a_refusal_to_retry_keeps_the_edit_and_the_rest_for_the_next_flush", async () => {
    // The registry's retry class decides: `now` (store_busy) and `later` (serve_limit) mean the
    // same request may yet land, so the edit stays pending, in order, with its own idem.
    for (const [code, retry] of [
      ["store_busy", "now"],
      ["serve_limit", "later"],
    ] as const) {
      const store = pendingStore(memory(), "w1");
      store.add(entry("u1", 1));
      store.add(entry("u2", 2));
      const { sent, transport } = fakeTransport(() =>
        Promise.reject(failure(code, "Busy.", "planroom doctor", retry)),
      );
      const flushed = await flush(transport, store);
      expect(
        sent.map((s) => s.idem),
        code,
      ).toEqual(["u1"]);
      expect(flushed.outcomes, code).toEqual([
        {
          idem: "u1",
          state: "held",
          message: "Busy.",
          hint: "planroom doctor",
        },
      ]);
      expect(
        store.list().map((e) => e.idem),
        code,
      ).toEqual(["u1", "u2"]);
    }
  });

  test("an_unreachable_engine_keeps_the_rest_in_order", async () => {
    const store = pendingStore(memory(), "w1");
    store.add(entry("u3", 3));
    store.add(entry("u1", 1));
    store.add(entry("u2", 2));
    const { sent, transport } = fakeTransport((_s, n) =>
      n === 1
        ? Promise.resolve(reply(false))
        : Promise.reject(new TransportError("unreachable", "down")),
    );
    const flushed = await flush(transport, store);
    expect(sent.map((s) => s.idem)).toEqual(["u1", "u2"]);
    expect(flushed.outcomes.map((o) => o.state)).toEqual(["landed"]);
    expect(flushed.stopped?.kind).toBe("unreachable");
    expect(store.list().map((e) => e.idem)).toEqual(["u2", "u3"]);
  });

  test("an_error_that_is_not_the_engines_is_rethrown", async () => {
    const store = pendingStore(memory(), "w1");
    store.add(entry("u1", 1));
    const { transport } = fakeTransport(() =>
      Promise.reject(new TypeError("bug")),
    );
    await expect(flush(transport, store)).rejects.toThrow("bug");
    expect(store.list()).toHaveLength(1);
  });
});
