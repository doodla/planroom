/**
 * The pane's view stack: what a read the person went down into showed (architecture.md, "Pane";
 * DR-85). A view holds the rows of one reply, the list draws the top one in place of the blocks,
 * and `z` takes one off. Pure: this file keeps and reads views and draws nothing.
 */

import { sanitize } from "@planroom/ui";

import { lineFor, oneLine, type Ran } from "./room";

/** One row of a view: a map's block, whose ID `x` goes down into, or a plain line (`id` null). */
export interface ViewRow {
  id: string | null;
  name: string;
}

/** What one successful read showed, kept until the pane's `seq` moves on. */
export interface View {
  /** The leaf that ran and its arguments after the leaf's name. */
  command: string;
  args: string[];
  /** The pane's `seq` when it ran: a newer pane makes the view stale. */
  seq: number;
  /** The engine's summary line, sanitized: the list's title. */
  title: string;
  rows: ViewRow[];
  /** For a map, the level it showed: only what the engine named, never a level of the mod's own. */
  level: string | null;
  /** The summary line, as `lineFor` says it. */
  line: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

/** `value` as a list of records, when it is one. */
function listOf(value: unknown): Record<string, unknown>[] | null {
  return Array.isArray(value) && value.every(isRecord) ? value : null;
}

/**
 * The view a successful run of `handle` showed at the pane's `seq`: a map's blocks with their IDs
 * and names, a plan's items as plain rows, or lint's findings as `name — message`. A failed run, or
 * a result that is none of these, is no view.
 */
export function viewFrom(
  handle: { command: string; args: readonly string[]; label: string },
  ran: Ran,
  seq: number,
): View | null {
  if (!ran.result.ok) return null;
  const result: unknown = ran.result.result;
  if (!isRecord(result)) return null;
  const rows = ((): { rows: ViewRow[]; level: string | null } | null => {
    const nodes = listOf(result["nodes"]);
    if (nodes !== null && typeof result["level"] === "string") {
      return {
        level: sanitize(result["level"]),
        rows: nodes.flatMap((n) =>
          typeof n["id"] === "string" && typeof n["name"] === "string"
            ? [{ id: n["id"], name: oneLine(n["name"]) }]
            : [],
        ),
      };
    }
    const findings = listOf(result["findings"]);
    if (findings !== null) {
      return {
        level: null,
        rows: findings.flatMap((f) =>
          typeof f["name"] === "string" && typeof f["message"] === "string"
            ? [{ id: null, name: oneLine(`${f["name"]} — ${f["message"]}`) }]
            : [],
        ),
      };
    }
    const items = listOf(result["items"]);
    if (items !== null) {
      return {
        level: null,
        rows: items.flatMap((i) =>
          typeof i["name"] === "string"
            ? [{ id: null, name: oneLine(i["name"]) }]
            : [],
        ),
      };
    }
    return null;
  })();
  if (rows === null) return null;
  const line = lineFor(handle.command, handle.label, ran);
  return {
    command: handle.command,
    args: [...handle.args],
    seq,
    title: line,
    rows: rows.rows,
    level: rows.level,
    line,
  };
}

/** The stack, or nothing once the pane's `seq` is not the top view's: views clear when it moves. */
export function current(views: readonly View[], seq: number): readonly View[] {
  return views.at(-1)?.seq === seq ? views : [];
}
