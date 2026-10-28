import { expect, test } from "vitest";

import mapReply from "../../../tests/golden/cli/map-graph.json";
import { parseReply, toolResult } from "./engine";
import {
  lineFor,
  replySummary,
  roomRows,
  roomText,
  ROOM_TEXT_CHARS,
  shownFrom,
  shows,
  type Ran,
} from "./room";

/** A reply as the mod keeps it, from the engine's stdout. */
function ran(stdout: string): Ran {
  return {
    result: toolResult({ exitCode: 0, stdout, stderr: "" }),
    summary: replySummary(parseReply(stdout)),
  };
}

const MAP = JSON.stringify(mapReply);
/** The golden reply as an engine without the envelope's `summary` sends it: the mod's own line. */
const { summary: GOLDEN_SUMMARY, ...withoutSummary } = mapReply;
const BARE_MAP = JSON.stringify(withoutSummary);
const OSC52 = "\u001b]52;c;aGVsbG8=\u0007";
const RLO = String.fromCodePoint(0x202e);

/** No control or bidi character survives into what the person sees. */
function clean(text: string): boolean {
  return !hasRaw(text);
}

/** Whether a control or bidi character other than a line break is left raw in `text`. */
function hasRaw(text: string): boolean {
  return [...text].some((c) => {
    const n = c.codePointAt(0) ?? 0;
    return (
      (n < 0x20 && n !== 0x0a) ||
      (n >= 0x7f && n <= 0x9f) ||
      (n >= 0x202a && n <= 0x202e) ||
      (n >= 0x2066 && n <= 0x2069)
    );
  });
}

test("room_text_within_budget: /room text is the summary line and handles, never JSON, within 300 tokens", () => {
  // The golden map is 2 KB of JSON; its /room text is one line the model can read.
  expect(MAP.length).toBeGreaterThan(ROOM_TEXT_CHARS);
  const text = roomText("map", ran(MAP));
  expect(text).toBe(GOLDEN_SUMMARY);
  expect(() => JSON.parse(text) as unknown).toThrow();
  // With no summary in the envelope, the mod's own line, still never JSON.
  expect(roomText("map", ran(BARE_MAP))).toBe("10 blocks at the system level");
  // The engine's own summary line, when the envelope carries one, and the handles to go on.
  const withSummary = JSON.stringify({
    ...mapReply,
    summary: "10 at the top: kit (11 code, 4 component); orders (3 code)",
    meta: {
      ...mapReply.meta,
      next: [
        {
          command: "map",
          args: ["--level", "container"],
          label: "Open the containers",
        },
      ],
    },
  });
  expect(roomText("map", ran(withSummary))).toBe(
    "10 at the top: kit (11 code, 4 component); orders (3 code)\n→ Open the containers: `/room map --level container`",
  );
  // However long the summary and however many handles, the text stays within the budget.
  const huge = JSON.stringify({
    ...mapReply,
    summary: "x".repeat(10_000),
    meta: {
      next: Array.from({ length: 50 }, (_, i) => ({
        command: "map",
        args: ["--level", `l${i}`, "y".repeat(400)],
        label: "z".repeat(300),
      })),
    },
  });
  const capped = roomText("map", ran(huge));
  expect(capped.length).toBeLessThanOrEqual(ROOM_TEXT_CHARS);
  expect(Math.ceil(capped.length / 4)).toBeLessThanOrEqual(300);
});

test("a failed /room says the engine's message and hint, never the error object", () => {
  const refused = JSON.stringify({
    schema_version: 1,
    ok: false,
    command: "map",
    error: {
      code: "invalid_argument",
      category: "usage",
      retry: "never",
      message: "--focus isn't available yet; run map without it.",
      hint: "planroom map",
    },
    meta: { seq: 0 },
  });
  expect(roomText("map", ran(refused))).toBe(
    "--focus isn't available yet; run map without it. · planroom map",
  );
  // No reply at all: the engine's first stderr line, as the tool result carries it.
  const none: Ran = {
    result: toolResult({ exitCode: 101, stdout: "", stderr: "panicked\nmore" }),
    summary: null,
  };
  expect(roomText("map", none)).toBe("panicked");
});

test("an empty level says its empty state's next action", () => {
  const empty = JSON.stringify({
    ...withoutSummary,
    result: {
      ...mapReply.result,
      level: "intent",
      nodes: [],
      empty: {
        reason: "no_intent",
        next: { command: "map", args: [], label: "Back to the system level" },
      },
    },
  });
  expect(lineFor("map", "map", ran(empty))).toBe("Back to the system level");
  const bare = JSON.stringify({
    ...withoutSummary,
    result: { ...mapReply.result, level: "intent", nodes: [], empty: null },
  });
  expect(lineFor("map", "map", ran(bare))).toBe(
    "No blocks at the intent level",
  );
});

test("an_empty_map_says_its_empty_states_line_before_its_label", () => {
  const unborn = (line: string) =>
    JSON.stringify({
      ...withoutSummary,
      result: {
        ...mapReply.result,
        nodes: [],
        empty: {
          reason: "repos_unborn",
          line,
          next: { command: "focus", args: [], label: "Check again" },
        },
      },
    });
  expect(
    lineFor(
      "map",
      "map",
      ran(
        unborn(
          "No commits yet in worker. Make a first commit, then check again.",
        ),
      ),
    ),
  ).toBe("No commits yet in worker. Make a first commit, then check again.");
  // The line is engine text over a repo's name, so it passes the display-sanitizing pass.
  const said = lineFor(
    "map",
    "map",
    ran(unborn(`No commits yet in evil${RLO}abc${OSC52}.`)),
  );
  expect(said).toContain("⟨U+202E⟩");
  expect(said).toContain("\\x1b]52");
  expect(said).not.toContain(RLO);
  expect(said).not.toContain("\u001b");
});

test("security-5: an OSC 52 or RLO in a name or the summary never reaches the transcript raw", () => {
  const evil = `evil${OSC52}name${RLO}abc`;
  const reply = JSON.stringify({
    ...mapReply,
    summary: `1 at the top: ${evil}`,
    result: {
      ...mapReply.result,
      nodes: [{ ...mapReply.result.nodes[0], name: evil }],
    },
    meta: { next: { command: "show", args: [evil], label: evil } },
  });
  const text = roomText("map", ran(reply));
  expect(clean(text)).toBe(true);
  expect(text).toContain("\\x1b]52");
  expect(text).toContain("⟨U+202E⟩");
  for (const row of roomRows("map", ran(reply))) {
    expect(clean(row.kind === "code" ? row.source : row.text), row.key).toBe(
      true,
    );
  }
  // A result in any other shape draws as JSON, which escapes controls but not bidi marks.
  const other = JSON.stringify({
    schema_version: 1,
    ok: true,
    command: "who",
    result: { rows: [{ name: evil }] },
    meta: { seq: 1 },
  });
  const code = roomRows("who", ran(other)).find((r) => r.kind === "code");
  expect(code?.kind === "code" && clean(code.source)).toBe(true);
});

test("/room draws a map's full result: one row a block, then the handles", () => {
  const rows = roomRows("map", ran(MAP));
  expect(rows[0]).toMatchObject({ key: "summary", text: GOLDEN_SUMMARY });
  expect(roomRows("map", ran(BARE_MAP))[0]).toMatchObject({
    key: "summary",
    text: "10 blocks at the system level",
  });
  const blocks = rows.filter((r) => r.key.startsWith("block-"));
  expect(blocks).toHaveLength(mapReply.result.nodes.length);
  // kit's four directories, its package and its two authors; its library is the container.
  expect(blocks[0]).toMatchObject({
    text: "□ kit · 11 code, 7 component, 1 container",
  });
});

test("a pressed next action is shown until the pane's seq moves on", () => {
  const handle = {
    command: "map",
    args: ["--level", "container"],
    label: "Open the containers",
  };
  const shown = shownFrom(handle, ran(MAP), 5);
  expect(shown).toMatchObject({ level: "system", line: GOLDEN_SUMMARY });
  expect(shown?.blocks).toContain("kit");
  expect(shows(shown, handle, 5)).toBe(true);
  expect(shows(shown, handle, 6)).toBe(false);
  expect(shows(shown, { command: "map", args: [] }, 5)).toBe(false);
  const failed: Ran = {
    result: toolResult({ exitCode: 2, stdout: "", stderr: "no" }),
    summary: null,
  };
  expect(shownFrom(handle, failed, 5)).toBeNull();
});
