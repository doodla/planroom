/**
 * `/room` arguments: a catalogue command line, parsed the way a shell splits words, against the
 * generated `ROOM_COMMANDS`, which also gives `/room help` and the typeahead (DR-14).
 */

import { MOD_STRINGS, type RoomCommand } from "@planroom/contract";
import { fill } from "@planroom/ui";

export type RoomRequest =
  /** `/room` alone: focus the pane. */
  | { kind: "focus" }
  | { kind: "help" }
  /** A `/room` command and the CLI arguments after its name. */
  | { kind: "run"; leaf: RoomCommand; args: string[] }
  | { kind: "error"; message: string };

/** Split `text` into words: whitespace separates, quotes group, a backslash escapes. */
export function tokenize(text: string): string[] | { error: string } {
  const words: string[] = [];
  let word = "";
  let inWord = false;
  let quote: '"' | "'" | null = null;
  for (let i = 0; i < text.length; i++) {
    const c = text.charAt(i);
    if (quote === "'") {
      if (c === "'") quote = null;
      else word += c;
    } else if (quote === '"') {
      if (c === '"') quote = null;
      else if (c === "\\" && (text[i + 1] === '"' || text[i + 1] === "\\"))
        word += text.charAt(++i);
      else word += c;
    } else if (c === "'" || c === '"') {
      quote = c;
      inWord = true;
    } else if (c === "\\" && i + 1 < text.length) {
      word += text.charAt(++i);
      inWord = true;
    } else if (/\s/.test(c)) {
      if (inWord) words.push(word);
      word = "";
      inWord = false;
    } else {
      word += c;
      inWord = true;
    }
  }
  if (quote !== null) {
    return { error: fill(MOD_STRINGS.room_open_quote, { quote }) };
  }
  if (inWord) words.push(word);
  return words;
}

/** Parse `/room`'s argument string against the `/room` commands. */
export function parseRoom(
  args: string,
  commands: readonly RoomCommand[],
): RoomRequest {
  const words = tokenize(args);
  if (!Array.isArray(words)) {
    return { kind: "error", message: words.error };
  }
  if (words.length === 0) {
    return { kind: "focus" };
  }
  if (words.length === 1 && words[0] === "help") {
    return { kind: "help" };
  }
  // The longest command name that the leading words spell, so `handoff merge` beats `handoff`.
  let best: RoomCommand | undefined;
  let bestLength = 0;
  for (const c of commands) {
    const path = c.name.split(" ");
    if (path.length > bestLength && path.every((p, i) => words[i] === p)) {
      best = c;
      bestLength = path.length;
    }
  }
  if (best === undefined) {
    return {
      kind: "error",
      message: fill(MOD_STRINGS.room_no_command, { word: words[0] ?? "" }),
    };
  }
  return { kind: "run", leaf: best, args: words.slice(bestLength) };
}

/**
 * The `/room help` text: `/room` alone and `/room help`, then each command's synopsis line, its
 * arguments set apart after the command's name, padded to the longest name.
 */
export function helpText(commands: readonly RoomCommand[]): string {
  const rows: [string, string][] = [
    ["/room", MOD_STRINGS.room_help_bare],
    ["/room help", MOD_STRINGS.room_help_help],
    ...commands.map((c): [string, string] => {
      const name = `/room ${c.name}`;
      const rest = c.synopsis.startsWith(name)
        ? c.synopsis.slice(name.length).trim()
        : c.synopsis;
      return [name, rest];
    }),
  ];
  const width = Math.max(...rows.map(([name]) => name.length));
  return rows
    .map(([name, text]) => `${name.padEnd(width)}  ${text}`.trimEnd())
    .join("\n");
}

export interface Suggestion {
  text: string;
  label: string;
  description: string;
}

/** A command word or a subcommand, offered once with its summary when it completes a command. */
function wordRows(
  done: readonly string[],
  partial: string,
  commands: readonly RoomCommand[],
): Suggestion[] {
  const rows = new Map<string, Suggestion>();
  const candidates =
    done.length === 0
      ? [{ name: "help", summary: MOD_STRINGS.room_help_help }, ...commands]
      : commands;
  for (const c of candidates) {
    const path = c.name.split(" ");
    if (path.length <= done.length || !done.every((w, i) => path[i] === w)) {
      continue;
    }
    const next = path[done.length] ?? "";
    if (next.startsWith(partial) && !rows.has(next)) {
      const whole = path.length === done.length + 1;
      rows.set(next, {
        text: next,
        label: `/room ${[...done, next].join(" ")}`,
        description: whole ? c.summary : "",
      });
    }
  }
  return [...rows.values()];
}

/**
 * Typeahead rows for the word at the cursor in a `/room ...` draft, in this order: the command
 * words that continue what's typed; then, past a command's name, its subcommand when it has one
 * and none is typed; then the flags not yet typed of the command and of the typed subcommand;
 * after a flag with a fixed set of values, those values, and after any other flag that takes a
 * value, nothing. No block names or IDs in M1. None outside
 * `/room`.
 */
export function completions(
  draft: string,
  cursor: number,
  commands: readonly RoomCommand[],
): Suggestion[] {
  const before = draft.slice(0, cursor);
  const match = /^\/room\s+(.*)$/s.exec(before);
  if (match === null) {
    return [];
  }
  const typed = match[1] ?? "";
  const words = typed.split(/\s+/);
  const partial = words.pop() ?? "";
  const done = words.filter((w) => w !== "");
  // The command the typed words spell, the longest first.
  let command: RoomCommand | undefined;
  let length = 0;
  for (const c of commands) {
    const path = c.name.split(" ");
    if (
      path.length > length &&
      path.length <= done.length &&
      path.every((p, i) => done[i] === p)
    ) {
      command = c;
      length = path.length;
    }
  }
  if (command === undefined) {
    return wordRows(done, partial, commands);
  }
  const rest = done.slice(length);
  const sub = rest.find((w) => command.subs.includes(w));
  const label = (word: string) => `/room ${[...done, word].join(" ")}`;
  // A flag that takes a value, just typed: its fixed values, or nothing when its value is open,
  // such as a level, which `ask` lists.
  const last = rest.at(-1);
  const valued = command.flags.find(
    (f) =>
      f.flag === last &&
      f.valueName !== null &&
      (f.sub === null || f.sub === sub),
  );
  if (valued !== undefined) {
    return valued.values
      .filter((v) => v.startsWith(partial))
      .map((v) => ({ text: v, label: label(v), description: "" }));
  }
  if (
    sub === undefined &&
    command.subs.length > 0 &&
    !partial.startsWith("-")
  ) {
    return command.subs
      .filter((s) => s.startsWith(partial))
      .map((s) => ({ text: s, label: label(s), description: "" }));
  }
  const rows = new Map<string, Suggestion>();
  for (const f of command.flags) {
    const open =
      (f.sub === null || f.sub === sub) &&
      f.flag.startsWith(partial) &&
      !rest.some((w) => w === f.flag || w.startsWith(`${f.flag}=`));
    if (open && !rows.has(f.flag)) {
      rows.set(f.flag, {
        text: f.flag,
        label: label(f.flag),
        description: f.valueName ?? "",
      });
    }
  }
  return [...rows.values()];
}
