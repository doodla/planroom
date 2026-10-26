/**
 * What a command's reply shows the person and the model (architecture.md, "`/room` family"; DR-65):
 * its summary line and the handles to go on, as the `/room` text the model reads, within 300
 * tokens; the full result as rows the `CommandOutput` site draws; and, for the pane's next action,
 * what it showed. Pure: register.tsx runs the commands. Every string a reply carries is
 * repo- or engine-derived, so each passes the display-sanitizing pass before it's shown (DR-64).
 */

import type { Envelope, Handle } from "@planroom/contract";
import { sanitize, truncate } from "@planroom/ui";

import { leaf } from "./catalogue";
import type { ToolResult } from "./engine";

/** One command's reply, as the mod keeps it: the tool result and the engine's summary line. */
export interface Ran {
  result: ToolResult;
  /** The engine's one-line summary of the reply, or null when the envelope has none. */
  summary: string | null;
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

/** A map reply's result, as far as the mod reads it. */
interface MapLike {
  level: string;
  nodes: { name?: unknown; counts?: unknown }[];
  empty?: { next?: unknown } | null;
}

function mapOf(result: unknown): MapLike | null {
  const r = result as Partial<MapLike> | null | undefined;
  return r !== null &&
    typeof r === "object" &&
    typeof r.level === "string" &&
    Array.isArray(r.nodes)
    ? (r as MapLike)
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
    return oneLine(said.length > 0 ? said.join(" · ") : `${label} failed`);
  }
  if (ran.summary !== null) return oneLine(ran.summary);
  const map = mapOf(result.result);
  if (map !== null) {
    const next = map.empty?.next;
    if (map.nodes.length === 0) {
      return oneLine(
        isHandle(next) ? next.label : `No blocks at the ${map.level} level`,
      );
    }
    const n = map.nodes.length;
    return oneLine(`${n} block${n === 1 ? "" : "s"} at the ${map.level} level`);
  }
  return oneLine(`${label || command}: done`);
}

/** The handles a reply offers to go on: its next actions, from `meta.next`, the pane, an empty state. */
export function handlesOf(result: ToolResult): Handle[] {
  const r = (result.ok ? result.result : null) as {
    next?: unknown;
    empty?: { next?: unknown } | null;
  } | null;
  const meta = result.meta["next"];
  const found = [
    ...(Array.isArray(meta) ? (meta as unknown[]) : [meta]),
    r?.empty?.next,
    r?.next,
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

/**
 * A `/room` result as the transcript row the model reads on every later turn: the summary line,
 * then each handle to go on, never the raw result; at most `ROOM_TEXT_CHARS` (DR-65).
 */
export function roomText(command: string, ran: Ran): string {
  const lines = [truncate(lineFor(command, command, ran), SUMMARY_CHARS)];
  for (const h of handlesOf(ran.result).slice(0, HANDLES)) {
    const line = `→ ${handleLine(h)}`;
    if (lines.join("\n").length + 1 + line.length > ROOM_TEXT_CHARS) break;
    lines.push(line);
  }
  return truncate(lines.join("\n"), ROOM_TEXT_CHARS);
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
  if (ran.result.ok) {
    const map = mapOf(ran.result.result);
    if (map !== null) {
      map.nodes.slice(0, ROOM_BLOCKS).forEach((n, i) => {
        const name = typeof n.name === "string" ? n.name : "";
        rows.push({
          kind: "text",
          key: `block-${i}`,
          text: oneLine(`□ ${name}${countsOf(n.counts)}`),
        });
      });
      if (map.nodes.length > ROOM_BLOCKS) {
        rows.push({
          kind: "text",
          key: "blocks-more",
          text: `+${map.nodes.length - ROOM_BLOCKS} more`,
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
  handlesOf(ran.result).forEach((h, i) => {
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
}

/** What a successful press of `handle` showed, for the pane to draw in place of the press. */
export function shownFrom(
  handle: { command: string; args: readonly string[]; label: string },
  ran: Ran,
  seq: number,
): Shown | null {
  if (!ran.result.ok) return null;
  const map = mapOf(ran.result.result);
  return {
    command: handle.command,
    args: [...handle.args],
    seq,
    line: lineFor(handle.command, handle.label, ran),
    level: map === null ? null : sanitize(map.level),
    blocks:
      map === null
        ? []
        : map.nodes.map((n) =>
            oneLine(typeof n.name === "string" ? n.name : ""),
          ),
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
