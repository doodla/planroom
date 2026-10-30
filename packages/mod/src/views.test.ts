import { expect, test } from "vitest";

import mapReply from "../../../tests/golden/cli/map-graph.json";
import { parseReply, toolResult } from "./engine";
import { replySummary, type Ran } from "./room";
import { current, viewFrom } from "./views";

/** A reply as the mod keeps it, from the engine's stdout. */
function ran(reply: unknown): Ran {
  const stdout = JSON.stringify(reply);
  return {
    result: toolResult({ exitCode: 0, stdout, stderr: "" }),
    summary: replySummary(parseReply(stdout)),
  };
}

/** A reply with `result` under the envelope. */
function reply(command: string, summary: string, result: unknown) {
  return {
    schema_version: 1,
    ok: true,
    command,
    summary,
    result,
    meta: { seq: 7 },
  };
}

const RLO = String.fromCodePoint(0x202e);

test("a_map_view_keeps_node_ids", () => {
  const handle = {
    command: "map",
    args: ["--focus", "orders:repo:orders"],
    label: "Down",
  };
  const view = viewFrom(handle, ran(mapReply), 7);
  expect(view).not.toBeNull();
  expect(view?.command).toBe("map");
  expect(view?.args).toEqual(["--focus", "orders:repo:orders"]);
  expect(view?.seq).toBe(7);
  // Every node keeps its ID beside its name, so a row can be the next focus.
  expect(view?.rows).toEqual(
    mapReply.result.nodes.map((n) => ({ id: n.id, name: n.name })),
  );
  expect(view?.rows.every((r) => typeof r.id === "string")).toBe(true);
  expect(view?.level).toBe(mapReply.result.level);
  expect(view?.line).toBe(mapReply.summary);
  expect(view?.title).toBe(mapReply.summary);
  // A failed run is no view.
  const failed: Ran = {
    result: toolResult({ exitCode: 2, stdout: "", stderr: "no" }),
    summary: null,
  };
  expect(viewFrom(handle, failed, 7)).toBeNull();
});

test("a_lint_view_lists_findings_as_rows", () => {
  const finding = {
    rule: "a_rule",
    stage: "lint",
    subject: "plan:app:a",
    name: `Add retries${RLO}`,
    message: "Names no outcome",
    values: [],
    related: [],
    handles: [],
  };
  const lint = viewFrom(
    { command: "intent lint", args: [], label: "Show the lint findings" },
    ran(
      reply("intent lint", "1 finding.", {
        project: "plan:app:a",
        summary: "1 finding.",
        findings: [finding],
      }),
    ),
    4,
  );
  expect(lint?.rows).toHaveLength(1);
  // A finding is a plain row: no ID, so no row key can select it.
  expect(lint?.rows[0]?.id).toBeNull();
  expect(lint?.rows[0]?.name).toContain(" — Names no outcome");
  expect(lint?.rows[0]?.name).toContain("Add retries");
  // No bidi control survives into a row.
  expect(lint?.rows[0]?.name).not.toContain(RLO);
  expect(lint?.level).toBeNull();

  const list = viewFrom(
    {
      command: "intent list",
      args: ["--project", "plan:app:a"],
      label: "Plan",
    },
    ran(
      reply("intent list", "2 items.", {
        summary: "2 items.",
        projects: [],
        items: [
          { id: "plan:app:a:1", name: "Intent" },
          { id: "plan:app:a:2", name: "Outcome" },
        ],
      }),
    ),
    4,
  );
  // An item names itself; it is not a block row either.
  expect(list?.rows).toEqual([
    { id: null, name: "Intent" },
    { id: null, name: "Outcome" },
  ]);
});

test("views_clear_when_the_seq_moves", () => {
  const view = viewFrom(
    { command: "map", args: ["--focus", "a:b:c"], label: "Down" },
    ran(mapReply),
    7,
  );
  if (view === null) throw new Error("no view");
  const stack = [view];
  expect(current(stack, 7)).toBe(stack);
  expect(current(stack, 8)).toEqual([]);
  expect(current([], 7)).toEqual([]);
});
