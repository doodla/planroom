/**
 * What a command's reply shows the person and the model (architecture.md, "`/room` family"; DR-65):
 * its summary line and the handles to go on, as the `/room` text the model reads, within 300
 * tokens; the full result as rows the `CommandOutput` site draws; and, for the pane's next action,
 * what it showed. Pure: register.tsx runs the commands. Every string a reply carries is
 * repo- or engine-derived, so each passes the display-sanitizing pass before it's shown (DR-64).
 */

import {
  MOD_COMMANDS,
  MOD_STRINGS,
  type Envelope,
  type Handle,
  type MapView,
  type Warning,
} from "@planroom/contract";
import { fill, sanitize, truncate } from "@planroom/ui";

import { leaf } from "./catalogue";
import type { ToolResult } from "./engine";
import { helpText, parseRoom } from "./room-args";
import type { RoomCommand } from "@planroom/contract";

/** One command's reply, as the mod keeps it: the tool result and the engine's summary line. */
export interface Ran {
  result: ToolResult;
  /** The engine's one-line summary of the reply, or null when the envelope has none. */
  summary: string | null;
  /** The arguments that ran after the command's name, for the next page's handle. */
  args?: readonly string[];
}

/** `/room`, as register.tsx registers it: immediate, so it focuses the pane mid-turn (DR-77). */
export const ROOM_COMMAND = {
  name: MOD_COMMANDS[0],
  description: MOD_STRINGS.room_description,
  argumentHint: MOD_STRINGS.room_argument_hint,
  immediate: true,
} as const;

/**
 * What one `/room` does, before anything runs: bare `/room` only opens and focuses the pane, with
 * no run, no text and no sync (DR-77); `help` and an error are text; a command line is a run.
 */
export function roomPlan(
  args: string,
  commands: readonly RoomCommand[],
): {
  open: boolean;
  run?: { command: string; args: string[] };
  text?: string;
} {
  const request = parseRoom(args, commands);
  switch (request.kind) {
    case "focus":
      return { open: true };
    case "help":
      return { open: false, text: helpText(commands) };
    case "error":
      return { open: false, text: sanitize(request.message) };
    case "run":
      return {
        open: false,
        run: { command: request.leaf.name, args: request.args },
      };
  }
}

/**
 * The engine's one-line summary of a `--json` reply: the envelope's optional `summary` (the
 * leaf's text-mode line), or null when it has none.
 */
export function replySummary(reply: Envelope | null): string | null {
  const summary = reply?.summary;
  return typeof summary === "string" && summary.trim() !== "" ? summary : null;
}

/** A string from a reply as one sanitized line: line breaks read ` · `, controls show as escapes. */
export function oneLine(text: string): string {
  return sanitize(text.trim().replace(/\s*[\r\n]+\s*/g, " · "));
}

/**
 * A reply's result as the contract's `MapView`, when it is one (drift-3-2): its level is a string
 * and each node has a name. What else the mod reads (`counts`, `empty.next`) it checks where it
 * reads it, since a reply is the engine's output, not the mod's own.
 */
function mapOf(result: unknown): MapView | null {
  const r = result as Partial<MapView> | null | undefined;
  return r !== null &&
    typeof r === "object" &&
    typeof r.level === "string" &&
    Array.isArray(r.nodes) &&
    r.nodes.every((n: unknown) => {
      const node = n as Partial<MapView["nodes"][number]> | null;
      return (
        node !== null &&
        typeof node === "object" &&
        typeof node.name === "string"
      );
    })
    ? (r as MapView)
    : null;
}

function isHandle(value: unknown): value is Handle {
  const h = value as Partial<Handle> | null | undefined;
  return (
    h !== null &&
    typeof h === "object" &&
    typeof h.command === "string" &&
    typeof h.label === "string" &&
    Array.isArray(h.args) &&
    h.args.every((a) => typeof a === "string")
  );
}

/** The handle that takes back what a reply did, such as an edit's `undo`, or null. */
export function undoOf(ran: Ran): Handle | null {
  if (!ran.result.ok) return null;
  const undo: unknown = (ran.result.result as { undo?: unknown } | null)?.undo;
  return isHandle(undo) ? undo : null;
}

/**
 * The reply's summary line: the engine's own, else a short line the mod can say without the
 * engine (an error's message and hint, a map's block count, or the label of what ran). Never JSON.
 */
export function lineFor(command: string, label: string, ran: Ran): string {
  const { result } = ran;
  if (!result.ok) {
    const e = result.error;
    const said = [e?.message, e?.hint ?? undefined].filter(
      (s): s is string => typeof s === "string" && s !== "",
    );
    return oneLine(
      said.length > 0
        ? said.join(" · ")
        : fill(MOD_STRINGS.failed_line, { label }),
    );
  }
  if (ran.summary !== null) return oneLine(ran.summary);
  const map = mapOf(result.result);
  if (map !== null) {
    const next = map.empty?.next;
    const said: unknown = map.empty?.line;
    if (map.nodes.length === 0) {
      // The empty state's line says why, where its action's label alone ("Check again") doesn't.
      if (typeof said === "string" && said !== "") return oneLine(said);
      return oneLine(
        isHandle(next)
          ? next.label
          : fill(MOD_STRINGS.no_blocks_at_level, { level: map.level }),
      );
    }
    // A page of a larger map counts the whole map, from its `truncated` warning's total.
    const total = warningsOf(result)
      .filter((w) => w.code === "truncated")
      .map((w) => (w.details as { total?: unknown } | null)?.total)
      .find((t): t is number => typeof t === "number");
    const n = total ?? map.nodes.length;
    return oneLine(
      fill(
        n === 1 ? MOD_STRINGS.blocks_at_level_one : MOD_STRINGS.blocks_at_level,
        { count: n, level: map.level },
      ),
    );
  }
  return oneLine(fill(MOD_STRINGS.done_line, { label: label || command }));
}

/** The warnings a reply carries, each with a string code and message. */
export function warningsOf(result: ToolResult): Warning[] {
  const sent: unknown = result.meta.warnings;
  return Array.isArray(sent)
    ? (sent as unknown[]).filter((w): w is Warning => {
        const x = w as Partial<Warning> | null;
        return (
          x !== null &&
          typeof x === "object" &&
          typeof x.code === "string" &&
          typeof x.message === "string"
        );
      })
    : [];
}

/** `args` less any `--cursor`, in either form. */
function withoutCursor(args: readonly string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const a = args[i] ?? "";
    if (a === "--cursor") {
      i++;
    } else if (!a.startsWith("--cursor=")) {
      out.push(a);
    }
  }
  return out;
}

/**
 * The handles a reply offers to go on, deduplicated, in this order: the result's next action, its
 * empty state's, each warning's `details.next` (a map page's drill-down among them), then, when
 * `meta.next` is a cursor, the next page: the command that ran with its arguments and that cursor.
 * `meta.next` is a cursor, never a handle (contracts.md, DR-86).
 */
export function handlesOf(
  command: string,
  result: ToolResult,
  args: readonly string[] = [],
): Handle[] {
  const r = (result.ok ? result.result : null) as {
    next?: unknown;
    empty?: { next?: unknown } | null;
  } | null;
  const cursor: unknown = result.meta.next;
  const page: Handle[] =
    typeof cursor === "string" && cursor !== ""
      ? [
          {
            command,
            args: [...withoutCursor(args), "--cursor", cursor],
            label: MOD_STRINGS.next_page_label,
          },
        ]
      : [];
  const found = [
    r?.next,
    r?.empty?.next,
    ...warningsOf(result).map(
      (w) => (w.details as { next?: unknown } | null)?.next,
    ),
    ...page,
  ].filter(isHandle);
  const seen = new Set<string>();
  return found.filter((h) => {
    const key = [h.command, ...h.args].join("\0");
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function shellWord(word: string): string {
  return /^[\w@%+=:,./-]+$/.test(word)
    ? word
    : `'${word.replace(/'/g, "'\\''")}'`;
}

/** A handle as the line that runs it: `/room` for a leaf `/room` serves, else the CLI's. */
export function handleLine(h: Handle): string {
  const run = leaf(h.command)?.room === true ? "/room" : "planroom";
  const line = [run, h.command, ...h.args.map(shellWord)].join(" ");
  return `${oneLine(h.label)}: \`${sanitize(line)}\``;
}

/** At most this many tokens of a `/room` result go back as text the model reads (DR-65). */
export const ROOM_TEXT_TOKENS = 300;
/** The characters that budget allows, at the four characters a token the engine's estimate uses. */
export const ROOM_TEXT_CHARS = ROOM_TEXT_TOKENS * 4;
const SUMMARY_CHARS = 600;
const HANDLES = 3;
/** The warnings a `/room` text says, each one line of at most `WARNING_CHARS`. */
const WARNINGS = 2;
const WARNING_CHARS = 200;

/** A warning as one line of `/room` text: `! message`, sanitized and cut. */
function warningLine(w: Warning): string {
  return truncate(
    oneLine(fill(MOD_STRINGS.room_warning, { message: w.message })),
    WARNING_CHARS,
  );
}

/**
 * A `/room` result as the transcript row the model reads on every later turn, never the raw
 * result: the summary line; at most two of its warnings, one line each; each handle to go on; and
 * a line counting the warnings left out. A line that doesn't fit `ROOM_TEXT_CHARS` is dropped
 * (DR-65).
 */
export function roomText(command: string, ran: Ran): string {
  const lines = [truncate(lineFor(command, command, ran), SUMMARY_CHARS)];
  const fits = (line: string) =>
    lines.join("\n").length + 1 + line.length <= ROOM_TEXT_CHARS;
  const warnings = warningsOf(ran.result);
  let said = 0;
  for (const w of warnings.slice(0, WARNINGS)) {
    const line = warningLine(w);
    if (!fits(line)) break;
    lines.push(line);
    said++;
  }
  for (const h of handlesOf(command, ran.result, ran.args).slice(0, HANDLES)) {
    const line = `→ ${handleLine(h)}`;
    if (!fits(line)) break;
    lines.push(line);
  }
  if (said < warnings.length) {
    const more = oneLine(
      fill(MOD_STRINGS.room_more_warnings, { count: warnings.length - said }),
    );
    if (fits(more)) lines.push(more);
  }
  return truncate(lines.join("\n"), ROOM_TEXT_CHARS);
}

/**
 * What `/room share` hands the model beside its text: one context entry, the publish's arguments
 * and the `db` writes as compact JSON, display-sanitized, so the model makes the publish with no
 * further read (architecture.md, "`/room` family"). Only an ok `share`, whose own budget bounds it.
 */
export function shareContext(command: string, ran: Ran): string[] | undefined {
  if (command !== "share" || !ran.result.ok) return undefined;
  const r = ran.result.result as { publish?: unknown; db?: unknown } | null;
  if (r === null || typeof r !== "object" || r.publish === undefined) {
    return undefined;
  }
  const json = JSON.stringify({ publish: r.publish, db: r.db ?? [] });
  return [sanitize(fill(MOD_STRINGS.share_context, { json }))];
}

/** One row of a `/room` result drawn in the `CommandOutput` site. */
export type RoomRow =
  | { kind: "text"; key: string; text: string; tone?: "dim" | "bold" | "warn" }
  | { kind: "code"; key: string; source: string };

/** A map draws one row a block, up to this many; a result in any other shape as JSON up to this size. */
const ROOM_BLOCKS = 200;
const ROOM_JSON_CHARS = 20_000;

function countsOf(counts: unknown): string {
  if (counts === null || typeof counts !== "object") return "";
  const parts = Object.entries(counts as Record<string, unknown>)
    .filter(([, n]) => typeof n === "number")
    .map(([level, n]) => `${String(n)} ${level}`);
  return parts.length === 0 ? "" : ` · ${parts.join(", ")}`;
}

/** The full result of a `/room` run, as the rows its `CommandOutput` row draws in place of its text. */
export function roomRows(command: string, ran: Ran): RoomRow[] {
  const rows: RoomRow[] = [
    {
      kind: "text",
      key: "summary",
      text: lineFor(command, command, ran),
      tone: ran.result.ok ? "bold" : "warn",
    },
  ];
  // Every warning, one row each; a `truncated` one says only that the result was cut, so it's dim.
  warningsOf(ran.result).forEach((w, i) => {
    rows.push({
      kind: "text",
      key: `warning-${i}`,
      text: oneLine(fill(MOD_STRINGS.room_warning, { message: w.message })),
      tone: w.code === "truncated" ? "dim" : "warn",
    });
  });
  if (ran.result.ok) {
    const map = mapOf(ran.result.result);
    if (map !== null) {
      map.nodes.slice(0, ROOM_BLOCKS).forEach((n, i) => {
        rows.push({
          kind: "text",
          key: `block-${i}`,
          text: oneLine(`□ ${n.name}${countsOf(n.counts)}`),
        });
      });
      if (map.nodes.length > ROOM_BLOCKS) {
        rows.push({
          kind: "text",
          key: "blocks-more",
          text: fill(MOD_STRINGS.more_rows, {
            count: map.nodes.length - ROOM_BLOCKS,
          }),
          tone: "dim",
        });
      }
    } else if (ran.result.result !== undefined && ran.result.result !== null) {
      // JSON escapes controls but not bidi marks, so the pretty text passes the sanitizer too.
      const json = JSON.stringify(ran.result.result, null, 2);
      rows.push({
        kind: "code",
        key: "result",
        source: truncate(
          json.split("\n").map(sanitize).join("\n"),
          ROOM_JSON_CHARS,
        ),
      });
    }
  }
  handlesOf(command, ran.result, ran.args).forEach((h, i) => {
    rows.push({
      kind: "text",
      key: `handle-${i}`,
      text: `→ ${handleLine(h)}`,
      tone: "dim",
    });
  });
  return rows;
}

/** What the pane's next action showed when it ran, until the pane moves on (ux-1). */
export interface Shown {
  command: string;
  args: readonly string[];
  /** The pane's `seq` when it ran: a newer pane makes it stale, and the next action comes back. */
  seq: number;
  /** Its summary line, sanitized. */
  line: string;
  /** For a map: the level it showed and its blocks' names, sanitized. */
  level: string | null;
  blocks: string[];
  /** For a map with no nodes: its empty state's line, sanitized, when it has one. */
  empty?: string | null;
}

/** What a successful press of `handle` showed, for the pane to draw in place of the press. */
export function shownFrom(
  handle: { command: string; args: readonly string[]; label: string },
  ran: Ran,
  seq: number,
): Shown | null {
  if (!ran.result.ok) return null;
  const map = mapOf(ran.result.result);
  const empty: unknown = map?.empty?.line;
  return {
    command: handle.command,
    args: [...handle.args],
    seq,
    line: lineFor(handle.command, handle.label, ran),
    level: map === null ? null : sanitize(map.level),
    blocks: map === null ? [] : map.nodes.map((n) => oneLine(n.name)),
    empty: typeof empty === "string" && empty !== "" ? oneLine(empty) : null,
  };
}

/** Whether `shown` is the result of `handle`, run at the pane's `seq`. */
export function shows(
  shown: Shown | null,
  handle: { command: string; args: readonly string[] },
  seq: number,
): shown is Shown {
  return (
    shown !== null &&
    shown.seq === seq &&
    shown.command === handle.command &&
    shown.args.length === handle.args.length &&
    shown.args.every((a, i) => a === handle.args[i])
  );
}
