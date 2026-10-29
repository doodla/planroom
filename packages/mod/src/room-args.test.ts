import { expect, test } from "vitest";

import { roomCommands } from "./catalogue";
import { completions, helpText, parseRoom, tokenize } from "./room-args";

const leaves = roomCommands();

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
  // A leaf of two words, such as `intent new`, offers its first word once.
  const firsts = [...new Set(leaves.map((l) => l.name.split(" ")[0]))];
  expect(all).toEqual(["help", ...firsts]);
  expect(completions("/room intent ", 13, leaves).map((s) => s.text)).toEqual(
    leaves
      .filter((l) => l.name.startsWith("intent "))
      .map((l) => l.name.split(" ")[1]),
  );
  // Past a command's name, its flags: the decided typeahead offers them in ROOM_COMMANDS order.
  expect(completions("/room map --le", 14, leaves).map((s) => s.text)).toEqual([
    "--level",
    "--lens",
  ]);
  expect(completions("/roomy m", 8, leaves)).toEqual([]);
  expect(completions("hello /room m", 13, leaves)).toEqual([]);
  expect(completions("/room f", 7, leaves).map((s) => s.text)).toEqual([
    "find",
    "focus",
  ]);
  expect(completions("/room fo", 8, leaves)[0]).toMatchObject({
    label: "/room focus",
  });
});

test("typeahead offers a command's subcommand, then its flags, then a flag's fixed values", () => {
  const texts = (draft: string) =>
    completions(draft, draft.length, leaves).map((s) => s.text);
  // A leaf with subcommands offers them first, none typed yet.
  expect(texts("/room edit ")).toEqual(
    leaves.find((l) => l.name === "edit")?.subs,
  );
  expect(texts("/room edit re")).toEqual(["rename", "relate"]);
  // With one typed, the leaf's own flags and that subcommand's: relate takes --kind.
  expect(texts("/room edit relate a b --")).toContain("--kind");
  expect(texts("/room edit move a b --")).not.toContain("--kind");
  // The flags the mod supplies are never offered.
  for (const draft of ["/room edit relate a b --", "/room map --"]) {
    for (const supplied of ["--base", "--idem", "--client", "--json"]) {
      expect(texts(draft)).not.toContain(supplied);
    }
  }
  // A flag already typed, either form, is not offered again.
  expect(texts("/room map --level system --l")).toEqual(["--lens"]);
  expect(texts("/room map --level=system --l")).toEqual(["--lens"]);
  // After a flag with a fixed set of values, those values.
  expect(texts("/room map --detail ")).toEqual(["summary", "graph"]);
  expect(texts("/room map --detail g")).toEqual(["graph"]);
  // A level is pack data with no fixed set in the catalogue, so `--level` offers nothing; `ask`
  // alone lists the levels.
  expect(texts("/room map --level ")).toEqual([]);
  // No block names or IDs in M1: a positional gets nothing but flags.
  expect(texts("/room find wor")).toEqual([]);
});

test("help lists each command's synopsis and the two lines for /room alone and /room help", () => {
  const help = helpText(leaves).split("\n");
  expect(help[0]).toMatch(/^\/room\s+focus the Planroom pane$/);
  expect(help[1]).toMatch(/^\/room help\s+list these commands$/);
  expect(help).toHaveLength(leaves.length + 2);
  const map = help.find((l) => l.startsWith("/room map "));
  expect(map).toContain("[--level LEVEL]");
  expect(map).not.toContain("--json");
  // One column: every command's arguments start where the longest name ends.
  const width = Math.max(...leaves.map((l) => `/room ${l.name}`.length));
  for (const line of help.slice(2)) {
    // A command with no arguments, such as open, ends at its name.
    if (line.length > width) expect(line.slice(width, width + 2)).toBe("  ");
  }
});
