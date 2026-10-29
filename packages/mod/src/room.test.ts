import { expect, test } from "vitest";

import { MOD_COMMANDS } from "@planroom/contract";

import mapReply from "../../../tests/golden/cli/map-graph.json";
import { roomCommands } from "./catalogue";
import { parseReply, toolResult } from "./engine";
import {
  lineFor,
  replySummary,
  ROOM_COMMAND,
  roomPlan,
  roomRows,
  roomText,
  ROOM_TEXT_CHARS,
  ROOM_TEXT_TOKENS,
  shareContext,
  shownFrom,
  shows,
  type Ran,
} from "./room";
import { helpText } from "./room-args";

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
/**
 * The golden map's block count. The link snapshot makes the fixture's top level more than one
 * page, so it is the `truncated` warning's total, not the page's own blocks.
 */
const MAP_TOTAL =
  (
    mapReply.meta.warnings.find((w) => w.code === "truncated")?.details as
      { total?: number } | undefined
  )?.total ?? mapReply.result.nodes.length;
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
  // The golden is a page: its summary, the page's warning, then the handles to go on, the page
  // after among them.
  const lines = text.split("\n");
  expect(lines[0]).toBe(GOLDEN_SUMMARY);
  expect(lines[1]).toMatch(/^! /);
  expect(lines).toContain(
    `→ Next page: \`/room map --cursor ${String(mapReply.meta.next)}\``,
  );
  expect(text.length).toBeLessThanOrEqual(ROOM_TEXT_CHARS);
  expect(() => JSON.parse(text) as unknown).toThrow();
  // With no summary in the envelope, the mod's own line, still never JSON.
  expect(roomText("map", ran(BARE_MAP)).split("\n")[0]).toBe(
    `${String(MAP_TOTAL)} blocks at the system level`,
  );
  // The engine's own summary line, when the envelope carries one, and the handles to go on: the
  // result's next action. A whole map, one page, has no page handle.
  const withSummary = JSON.stringify({
    ...mapReply,
    meta: { seq: mapReply.meta.seq, warnings: [] },
    summary: "10 at the top: kit (11 code, 4 component); orders (3 code)",
    result: {
      ...mapReply.result,
      next: {
        command: "map",
        args: ["--level", "container"],
        label: "Open the containers",
      },
    },
  });
  expect(roomText("map", ran(withSummary))).toBe(
    "10 at the top: kit (11 code, 4 component); orders (3 code)\n→ Open the containers: `/room map --level container`",
  );
  // However long the summary and however many warnings and handles, the text stays within the
  // budget.
  const huge = JSON.stringify({
    ...mapReply,
    summary: "x".repeat(10_000),
    meta: {
      warnings: Array.from({ length: 50 }, (_, i) => ({
        code: "repo_unborn",
        message: "w".repeat(500),
        details: {
          next: {
            command: "map",
            args: ["--level", `l${i}`, "y".repeat(400)],
            label: "z".repeat(300),
          },
        },
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
    text: `${String(MAP_TOTAL)} blocks at the system level`,
  });
  const blocks = rows.filter((r) => r.key.startsWith("block-"));
  expect(blocks).toHaveLength(mapReply.result.nodes.length);
  // orders leads: 14 directories, its package, its API spec and its two authors; its 23 files and
  // the 5 endpoints the link snapshot makes of its routes.
  expect(blocks[0]).toMatchObject({
    text: "□ orders · 28 code, 18 component, 1 container",
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

test("room_is_immediate: /room registers as an immediate command, from MOD_COMMANDS", () => {
  expect(ROOM_COMMAND).toMatchObject({ name: "room", immediate: true });
  expect(ROOM_COMMAND.name).toBe(MOD_COMMANDS[0]);
});

test("bare_room_starts_no_sync: /room alone only opens the pane, with no run and no text", () => {
  for (const args of ["", "   ", "\t"]) {
    expect(roomPlan(args, roomCommands())).toEqual({ open: true });
  }
  expect(roomPlan("help", roomCommands())).toMatchObject({ open: false });
  expect(roomPlan("help", roomCommands()).run).toBeUndefined();
  expect(roomPlan("map --level system", roomCommands())).toEqual({
    open: false,
    run: { command: "map", args: ["--level", "system"] },
  });
});

test("room_commands_snapshot: the /room commands, their summaries and the two help lines", () => {
  expect(roomCommands().map((c) => [c.name, c.summary])).toEqual([
    ["map", "The map at a level through a lens."],
    ["find", "Search the map's blocks by name or ID, best match first."],
    [
      "ask",
      "A question the loaded packs answer; alone, every loaded kind, lens and question.",
    ],
    ["focus", "Where you are, the one open decision and the next action."],
    ["scan", "Discover, extract and link the workspace's repos."],
    [
      "share",
      "Write the canvas, its snapshot and the first live head for an Artifact publish.",
    ],
    [
      "edit",
      "One hand edit: move, group, rename, hide, pin, relate, unrelate or forbid, or undo a command.",
    ],
    [
      "open",
      "Open the canvas in your browser, signed in with a one-time code it never prints.",
    ],
    ["intent new", "Start a plan: a new project named by its text."],
    [
      "intent next",
      "Draft a plan stage's items with Claude; they wait as ghosts for a person.",
    ],
    [
      "intent propose",
      "Propose plan items from a file or stdin; an agent's wait as ghosts for a person.",
    ],
    ["intent lint", "The plan's lint findings: what each stage still lacks."],
    [
      "intent version",
      "Tag the plan as it stands with a version label; a person's alone.",
    ],
    [
      "intent list",
      "Every project, and the items of the one named, each ghost with its review.",
    ],
    [
      "pack add",
      "Add an out-of-tree pack: validate it, copy it into the state directory and record it; a person's alone.",
    ],
    [
      "pack remove",
      "Remove an out-of-tree pack from the workspace's pack set; a person's alone.",
    ],
  ]);
  expect(helpText(roomCommands()).split("\n").slice(0, 2)).toEqual([
    "/room                 focus the Planroom pane",
    "/room help            list these commands",
  ]);
  // Every synopsis is the leaf's own, under the strip rule.
  for (const c of roomCommands()) {
    expect(c.synopsis.startsWith(`/room ${c.name}`)).toBe(true);
    for (const supplied of ["[--json]", "[--base", "[--idem", "[--client"]) {
      expect(c.synopsis).not.toContain(supplied);
    }
  }
});

test("the /room text budget is budgets.json's room_text", () => {
  expect(__PLANROOM_BUDGETS__.room_text).toBe(ROOM_TEXT_TOKENS);
});

/** A scan's reply with one unborn repo and one another user owns. */
const PER_REPO = JSON.stringify({
  schema_version: 1,
  ok: true,
  command: "scan",
  summary: "Mapped 2 of 4 repos.",
  result: { map: { level: "system", nodes: [] } },
  meta: {
    seq: 31,
    warnings: [
      {
        code: "repo_unborn",
        message:
          "billing has no commit yet, so it maps nothing until its first commit.",
        details: { repo: "billing", branch: "main" },
      },
      {
        code: "repo_skipped",
        message:
          "legacy is owned by user 1001; Planroom never runs git in another user's repo.",
        details: { repo: "legacy", reason: "other_owner", fix: null },
      },
    ],
  },
});

test("room_shows_per_repo_warnings: each repo's warning is a line of /room text and a warn row", () => {
  const text = roomText("scan", ran(PER_REPO));
  expect(text.split("\n")).toEqual([
    "Mapped 2 of 4 repos.",
    "! billing has no commit yet, so it maps nothing until its first commit.",
    "! legacy is owned by user 1001; Planroom never runs git in another user's repo.",
  ]);
  expect(Math.ceil(text.length / 4)).toBeLessThanOrEqual(ROOM_TEXT_TOKENS);
  const warned = roomRows("scan", ran(PER_REPO)).filter(
    (r) =>
      r.kind === "text" && r.tone === "warn" && r.key.startsWith("warning-"),
  );
  expect(warned).toHaveLength(2);
  // A third says how many more were left out of the text, never all of them.
  const three = JSON.parse(PER_REPO) as { meta: { warnings: unknown[] } };
  three.meta.warnings.push({
    code: "repo_unborn",
    message: "docs has no commit yet.",
    details: { repo: "docs", branch: "main" },
  });
  const more = roomText("scan", ran(JSON.stringify(three)));
  expect(more.split("\n").at(-1)).toBe("! 1 more warnings: /room shows them");
  expect(more).not.toContain("docs has no commit");
});

test("a_truncated_map_shows_its_drill_down_and_next_page", () => {
  const drill = {
    command: "map",
    args: ["--level", "container"],
    label: "Open the containers",
  };
  const page = JSON.stringify({
    ...withoutSummary,
    result: { ...mapReply.result, level: "component" },
    meta: {
      seq: 9,
      next: "c_2",
      truncated: true,
      warnings: [
        {
          code: "truncated",
          message: "The map holds 40 blocks; this page shows 10.",
          details: { kept: 10, total: 40, budget_tokens: 1500, next: drill },
        },
      ],
    },
  });
  const reply: Ran = {
    ...ran(page),
    args: ["--level", "component", "--cursor=c_1"],
  };
  const text = roomText("map", reply).split("\n");
  // The whole map's count, from the warning, not the page's own blocks.
  expect(text[0]).toBe("40 blocks at the component level");
  expect(text).toContain("! The map holds 40 blocks; this page shows 10.");
  expect(text).toContain(
    "→ Open the containers: `/room map --level container`",
  );
  // The next page: the same command and arguments, its old cursor replaced.
  expect(text).toContain(
    "→ Next page: `/room map --level component --cursor c_2`",
  );
  const truncated = roomRows("map", reply).find((r) => r.key === "warning-0");
  expect(truncated).toMatchObject({ tone: "dim" });
});

test("room_share_gives_the_model_its_publish_arguments", () => {
  const publish = {
    file_path: "/home/you/shop/.planroom/share/page.html",
    root: "/home/you/shop/.planroom/share",
    files: { "data/snapshot.json": "data/snapshot.json" },
    capabilities: { db: {}, user: {} },
    title: "shop",
    description: `The map of shop ${RLO}`,
    icon: "map",
  };
  const db = [{ ops: [] }];
  const shared = JSON.stringify({
    schema_version: 1,
    ok: true,
    command: "share",
    summary: "Ready to publish shop's map.",
    result: { summary: "map", publish, db, files: [], holds: [] },
    meta: { seq: 12 },
  });
  const context = shareContext("share", ran(shared));
  expect(context).toHaveLength(1);
  const entry = context?.[0] ?? "";
  expect(entry).toContain(`"file_path":"${publish.file_path}"`);
  expect(entry).toContain('"db":[{"ops":[]}]');
  expect(clean(entry)).toBe(true);
  // Only an ok share hands any: not another command, not a failure.
  expect(shareContext("map", ran(MAP))).toBeUndefined();
  const failed: Ran = {
    result: toolResult({ exitCode: 2, stdout: "", stderr: "no" }),
    summary: null,
  };
  expect(shareContext("share", failed)).toBeUndefined();
  // The text stays the summary and handles.
  expect(roomText("share", ran(shared))).toBe("Ready to publish shop's map.");
});
