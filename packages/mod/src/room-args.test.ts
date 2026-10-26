import { expect, test } from "vitest";

import { roomLeaves } from "./catalogue";
import { completions, helpText, parseRoom, tokenize } from "./room-args";

const leaves = roomLeaves();

test("words split on whitespace, with quotes and escapes grouping", () => {
  expect(tokenize(`  map --level container `)).toEqual([
    "map",
    "--level",
    "container",
  ]);
  expect(tokenize(`intent "Add retries to the worker"`)).toEqual([
    "intent",
    "Add retries to the worker",
  ]);
  expect(tokenize(`a 'it''s' "say \\"hi\\"" b\\ c`)).toEqual([
    "a",
    "its",
    'say "hi"',
    "b c",
  ]);
  expect(tokenize(`""`)).toEqual([""]);
  expect(tokenize(`map "open`)).toEqual({ error: "A \" quote isn't closed." });
});

test("/room alone focuses the pane and help lists the commands", () => {
  expect(parseRoom("", leaves)).toEqual({ kind: "focus" });
  expect(parseRoom("   ", leaves)).toEqual({ kind: "focus" });
  expect(parseRoom("help", leaves)).toEqual({ kind: "help" });
  const help = helpText(leaves);
  for (const l of leaves) expect(help).toContain(`/room ${l.name}`);
});

test("a catalogue command line runs that leaf with its arguments", () => {
  const request = parseRoom("map --level container --lens calls", leaves);
  expect(request.kind).toBe("run");
  if (request.kind === "run") {
    expect(request.leaf.name).toBe("map");
    expect(request.args).toEqual(["--level", "container", "--lens", "calls"]);
  }
});

test("the longest leaf name wins", () => {
  const two = [...leaves, { ...leaves[0]!, name: "map extra" }];
  const request = parseRoom("map extra --x", two);
  expect(request.kind === "run" && request.leaf.name).toBe("map extra");
});

test("an unknown command or a broken quote is one line naming the fix", () => {
  expect(parseRoom("teleport", leaves)).toEqual({
    kind: "error",
    message: '/room has no command "teleport". /room help lists them.',
  });
  expect(parseRoom(`map "x`, leaves).kind).toBe("error");
});

test("typeahead offers the command words that continue the draft", () => {
  expect(completions("/room m", 7, leaves).map((s) => s.text)).toEqual(["map"]);
  const all = completions("/room ", 6, leaves).map((s) => s.text);
  expect(all).toEqual(["help", ...leaves.map((l) => l.name)]);
  expect(completions("/room map --le", 14, leaves)).toEqual([]);
  expect(completions("/roomy m", 8, leaves)).toEqual([]);
  expect(completions("hello /room m", 13, leaves)).toEqual([]);
  expect(completions("/room f", 7, leaves)[0]).toMatchObject({
    label: "/room focus",
  });
});
