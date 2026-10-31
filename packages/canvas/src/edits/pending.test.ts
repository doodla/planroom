import { describe, expect, test } from "vitest";

import { flush } from "./flush";
import { pendingKey, pendingStore, type PendingEdit } from "./pending";
import { tabId } from "./tab";
import type { Transport } from "../transport/types";

function memory() {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (k: string) => items.get(k) ?? null,
    setItem: (k: string, v: string) => void items.set(k, v),
    removeItem: (k: string) => void items.delete(k),
  };
}

const blocked = () => {
  const refuse = () => {
    throw new Error("blocked");
  };
  return { getItem: refuse, setItem: refuse, removeItem: refuse };
};

const edit = (
  idem: string,
  at = 1,
  over: Partial<PendingEdit> = {},
): PendingEdit => ({
  idem,
  client: "tab:0123456789abcdef",
  request: { op: "hide", id: "x", base: 7, client: "tab:0123456789abcdef" },
  line: "Hide x",
  at,
  ...over,
});

describe("pending edits", () => {
  test("a_pending_edit_keeps_its_idem_and_its_tabs_client", () => {
    const mem = memory();
    pendingStore(mem, "w1").add(edit("u1"));
    const read = pendingStore(mem, "w1").list();
    expect(read).toHaveLength(1);
    expect(read[0]?.idem).toBe("u1");
    expect(read[0]?.client).toBe("tab:0123456789abcdef");
    expect(read[0]?.request.base).toBe(7);

    const session = memory();
    const first = tabId(session);
    expect(first).toMatch(/^tab:[0-9a-f]{16}$/);
    expect(tabId(session)).toBe(first);
    expect(tabId(memory())).not.toBe(first);
  });

  test("a_tab_id_a_page_stored_is_reused_and_anything_else_is_replaced", () => {
    const session = memory();
    session.setItem("planroom.tab", "tab:aaaaaaaaaaaaaaaa");
    expect(tabId(session)).toBe("tab:aaaaaaaaaaaaaaaa");
    session.setItem("planroom.tab", "junk");
    expect(tabId(session)).toMatch(/^tab:[0-9a-f]{16}$/);
    expect(session.getItem("planroom.tab")).toMatch(/^tab:[0-9a-f]{16}$/);
  });

  test("two_stores_over_one_storage_interleave_per_call", () => {
    const mem = memory();
    const a = pendingStore(mem, "w1");
    const b = pendingStore(mem, "w1");
    a.add(edit("u2", 2));
    b.add(edit("u1", 1));
    a.add(edit("u0", 1));
    expect(a.list().map((e) => e.idem)).toEqual(["u0", "u1", "u2"]);
    b.remove("u1");
    expect(a.list().map((e) => e.idem)).toEqual(["u0", "u2"]);
  });

  test("a_stored_value_that_is_not_a_list_reads_as_empty", () => {
    const mem = memory();
    mem.setItem(pendingKey("w1"), "{not json");
    expect(pendingStore(mem, "w1").list()).toEqual([]);
    mem.setItem(pendingKey("w1"), '{"a":1}');
    expect(pendingStore(mem, "w1").list()).toEqual([]);
  });

  test("another_workspaces_pending_edits_are_never_sent", async () => {
    const mem = memory();
    pendingStore(mem, "w1").add(edit("u1"));
    let writes = 0;
    const fake = {
      write: () => {
        writes += 1;
        return Promise.reject(new Error("never"));
      },
    } as unknown as Transport;
    const flushed = await flush(fake, pendingStore(mem, "w2"));
    expect(writes).toBe(0);
    expect(flushed.outcomes).toEqual([]);
    expect(
      pendingStore(mem, "w1")
        .list()
        .map((e) => e.idem),
    ).toEqual(["u1"]);
  });

  test("blocked_storage_keeps_pending_edits_for_this_page", () => {
    const store = pendingStore(blocked(), "wb");
    store.add(edit("u1"));
    expect(store.list().map((e) => e.idem)).toEqual(["u1"]);
    expect(
      pendingStore(blocked(), "wb")
        .list()
        .map((e) => e.idem),
    ).toEqual(["u1"]);
    store.remove("u1");
    expect(pendingStore(blocked(), "wb").list()).toEqual([]);
  });
});
