import { expect, test } from "vitest";

import { SseParser } from "./sse";

test("a message ends at a blank line and keeps its event, id and data", () => {
  const p = new SseParser();
  expect(p.push('event: event\nid: 7\ndata: {"seq":7}\n')).toEqual([]);
  expect(p.push("\n")).toEqual([
    { event: "event", id: "7", data: '{"seq":7}' },
  ]);
  expect(p.lastEventId).toBe("7");
});

test("chunks may split anywhere, CRLF included, and comments are skipped", () => {
  const p = new SseParser();
  const text =
    ": keepalive\r\nevent: ping\r\ndata: {}\r\n\r\ndata: a\r\ndata: b\r\n\r\n";
  const out = [...text].flatMap((ch) => p.push(ch));
  expect(out).toEqual([
    { event: "ping", id: null, data: "{}" },
    { event: "message", id: null, data: "a\nb" },
  ]);
});

test("an event with no data dispatches nothing and resets its type", () => {
  const p = new SseParser();
  expect(p.push("event: lonely\n\ndata: x\n\n")).toEqual([
    { event: "message", id: null, data: "x" },
  ]);
});
