/** `/room` arguments: a catalogue command line, parsed the way a shell splits words. */

import type { CatalogueLeaf } from "@planroom/contract";

export type RoomRequest =
  /** `/room` alone: focus the pane. */
  | { kind: "focus" }
  | { kind: "help" }
  /** A catalogue leaf and the CLI arguments after its name. */
  | { kind: "run"; leaf: CatalogueLeaf; args: string[] }
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
    return { error: `A ${quote} quote isn't closed.` };
  }
  if (inWord) words.push(word);
  return words;
}

/** Parse `/room`'s argument string against the leaves `/room` reaches. */
export function parseRoom(
  args: string,
  leaves: readonly CatalogueLeaf[],
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
  // The longest leaf name that the leading words spell, so `handoff merge` beats `handoff`.
  let best: CatalogueLeaf | undefined;
  let bestLength = 0;
  for (const l of leaves) {
    const path = l.name.split(" ");
    if (path.length > bestLength && path.every((p, i) => words[i] === p)) {
      best = l;
      bestLength = path.length;
    }
  }
  if (best === undefined) {
    return {
      kind: "error",
      message: `/room has no command "${words[0]}". /room help lists them.`,
    };
  }
  return { kind: "run", leaf: best, args: words.slice(bestLength) };
}

/** The `/room help` text: one line per command. */
export function helpText(leaves: readonly CatalogueLeaf[]): string {
  const lines = [
    "/room            focus the Planroom pane",
    "/room help       list these commands",
    ...leaves.map((l) => `/room ${l.name.padEnd(10)} ${l.summary}`),
  ];
  return lines.join("\n");
}

export interface Suggestion {
  text: string;
  label: string;
  description: string;
}

/**
 * Typeahead rows for the word at the cursor in a `/room ...` draft: the command words that
 * continue what's typed. None outside `/room`, or past the command's name.
 */
export function completions(
  draft: string,
  cursor: number,
  leaves: readonly CatalogueLeaf[],
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
  const rows = new Map<string, Suggestion>();
  const candidates =
    done.length === 0
      ? [{ name: "help", summary: "List the /room commands." }, ...leaves]
      : leaves;
  for (const l of candidates) {
    const path = l.name.split(" ");
    if (path.length <= done.length || !done.every((w, i) => path[i] === w)) {
      continue;
    }
    const next = path[done.length] ?? "";
    if (next.startsWith(partial) && !rows.has(next)) {
      const whole = path.length === done.length + 1;
      rows.set(next, {
        text: next,
        label: `/room ${[...done, next].join(" ")}`,
        description: whole ? l.summary : "",
      });
    }
  }
  return [...rows.values()];
}
