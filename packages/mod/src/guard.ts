/**
 * The mod's `tool.check` guard for every agent (architecture.md, "Actors"; DR-52, DR-64, DR-75):
 * no reads of the stores, the serve file, the data root's stores, ledger, backups and login pages,
 * the user-level config or the mod's own `$.store` file that holds the person key; no writes under
 * a state directory or the data root; and no Bash that runs a person-only command, sets a variable
 * that hands out the person's authority or picks the engine, or pushes `refs/planroom/`.
 *
 * Pure: register.tsx resolves each path (relative, `..`, symlinks) and passes every spelling. It
 * is a guardrail for "AI proposes, person disposes", not a boundary between processes of one user.
 */

import { CATALOGUE } from "@planroom/contract";

/** Where Planroom keeps what an agent mustn't read or write, for this user. */
export interface GuardPlaces {
  /** The session's working directory, for relative paths. */
  cwd: string;
  home: string;
  /** `${CLAUDE_CONFIG_DIR:-$HOME/.claude}`, which holds the mod's `$.store` file [A-15]. */
  claudeConfig: string;
  /** `${PLANROOM_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/planroom}`. */
  dataRoot: string;
  /** `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml`, which may hold the gateway key. */
  userConfig: string;
}

export interface Denial {
  decision: "deny";
  reason: string;
}

/** The places from the environment variables the guard reads. */
export function guardPlaces(
  cwd: string,
  env: {
    HOME?: string | undefined;
    CLAUDE_CONFIG_DIR?: string | undefined;
    PLANROOM_DATA?: string | undefined;
    XDG_DATA_HOME?: string | undefined;
    XDG_CONFIG_HOME?: string | undefined;
  },
): GuardPlaces {
  const home = env.HOME ?? "/";
  const nonEmpty = (v: string | undefined): string | undefined =>
    v === undefined || v === "" ? undefined : v;
  return {
    cwd,
    home,
    claudeConfig: nonEmpty(env.CLAUDE_CONFIG_DIR) ?? `${home}/.claude`,
    dataRoot:
      nonEmpty(env.PLANROOM_DATA) ??
      `${nonEmpty(env.XDG_DATA_HOME) ?? `${home}/.local/share`}/planroom`,
    userConfig: `${nonEmpty(env.XDG_CONFIG_HOME) ?? `${home}/.config`}/planroom/config.toml`,
  };
}

/** An absolute, lexically normal path: `~` expanded, relative to `cwd`, `.` and `..` folded. */
export function normalize(path: string, cwd: string, home: string): string {
  let p = path.trim();
  if (p === "~" || p.startsWith("~/")) p = home + p.slice(1);
  if (!p.startsWith("/")) p = `${cwd}/${p}`;
  const parts: string[] = [];
  for (const part of p.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return `/${parts.join("/")}`;
}

function within(path: string, dir: string): boolean {
  const d = dir.replace(/\/+$/, "");
  return path === d || path.startsWith(`${d}/`);
}

function basename(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** Whether a path is in a workspace's state directory: `.planroom/`, or a repo's `.git/planroom/`. */
function inStateDir(path: string): boolean {
  return /(^|\/)(\.planroom|\.git\/planroom)(\/|$)/.test(path);
}

const STORE_FILE = /^(store|ledger)\.db(-wal|-shm|-journal)?$/;

/** Why an agent may not read this path (absolute and normal), or null. */
export function readDenied(path: string, places: GuardPlaces): string | null {
  const name = basename(path);
  if (STORE_FILE.test(name) || name === "store.pointer") {
    return "Planroom's store is read through the planroom CLI, never directly.";
  }
  if (inStateDir(path) && name === "serve.json") {
    return "serve.json holds the canvas server's control token.";
  }
  const data = places.dataRoot;
  for (const sub of ["stores", "backups", "open", "ledger.db"]) {
    if (within(path, `${data}/${sub}`)) {
      return "The Planroom data root's stores, ledger, backups and login pages are off limits.";
    }
  }
  if (path === places.userConfig) {
    return "The user-level Planroom config may hold the gateway key.";
  }
  if (
    within(path, places.claudeConfig) &&
    !within(path, `${places.claudeConfig}/plugins`) &&
    /planroom/i.test(path.slice(places.claudeConfig.length))
  ) {
    // A-15: until WS8 pins the $.store file's path, every file there whose name or folder names
    // planroom is kept out, except the installed plugins' own files.
    return "The Planroom mod's store holds the person key.";
  }
  return null;
}

/** Why an agent may not write this path (absolute and normal), or null. */
export function writeDenied(path: string, places: GuardPlaces): string | null {
  if (inStateDir(path)) {
    return "Planroom's state directory changes only through the planroom CLI.";
  }
  if (within(path, places.dataRoot)) {
    return "The Planroom data root changes only through the planroom CLI.";
  }
  return readDenied(path, places);
}

/** The words of a shell command, quotes and operators taken away, so `sh -c '...'` shows through. */
export function shellWords(command: string): string[] {
  return command
    .replace(/\\\n/g, " ")
    .split(/[\s;&|()<>`'"=$]+/)
    .filter((w) => w !== "");
}

/** Person-only leaves the catalogue doesn't hold yet, from the leaf table in contracts.md. */
const PERSON_ONLY: readonly string[][] = [
  ["intent", "version"],
  ["handoff", "start"],
  ["handoff", "merge"],
  ["handoff", "revert"],
  ["handoff", "cancel"],
  ["pack", "add"],
  ["pack", "remove"],
  ["person", "init"],
  ["person", "pair"],
  ["open"],
];

function personOnlyPaths(): string[][] {
  const fromCatalogue = CATALOGUE.filter((l) => l.actor === "person_only").map(
    (l) => l.name.split(" "),
  );
  return [...PERSON_ONLY, ...fromCatalogue];
}

/** Global and write options that take a value, so the words after them aren't the command. */
const VALUE_FLAGS = new Set([
  "--workspace",
  "--limit",
  "--cursor",
  "--base",
  "--idem",
  "--project",
]);

/** The command words that follow each `planroom` in a shell line, with its own options skipped. */
export function planroomLines(words: readonly string[]): string[][] {
  const lines: string[][] = [];
  words.forEach((word, i) => {
    if (basename(word) !== "planroom") return;
    const rest: string[] = [];
    for (let j = i + 1; j < words.length; j++) {
      const w = words[j] ?? "";
      if (basename(w) === "planroom") break;
      if (w.startsWith("--")) {
        if (VALUE_FLAGS.has(w)) j++;
        rest.push(w);
        continue;
      }
      rest.push(w);
    }
    lines.push(rest);
  });
  return lines;
}

function positionals(line: readonly string[]): string[] {
  return line.filter((w) => !w.startsWith("-"));
}

/** Why a `planroom` line is the person's alone, or null. */
export function personOnlyLine(line: readonly string[]): string | null {
  const words = positionals(line);
  for (const path of personOnlyPaths()) {
    if (path.every((p, i) => words[i] === p)) {
      return `planroom ${path.join(" ")} is the person's to run: ask them to press it in the Planroom pane.`;
    }
  }
  // `decide` lists and runs for anyone; `decide ID CHOICE` is a person's pick.
  if (words[0] === "decide" && words.length >= 3) {
    return "Picking a decision card is the person's: ask them to press it in the Planroom pane.";
  }
  if (words[0] === "doctor" && line.includes("--restore")) {
    return "planroom doctor --restore is the person's to run.";
  }
  return null;
}

/**
 * Variables an agent never sets: the person's token, raw output, and the two that choose the
 * binary every later session runs (DR-64, DR-69).
 */
const GUARDED_VARIABLES =
  /\bPLANROOM_(PERSON_TOKEN|RAW|ENGINE|RELEASE_BASE_URL)\b\s*=|\b(export|declare|typeset|env|set)\b[^;&|\n]*\bPLANROOM_(PERSON_TOKEN|RAW|ENGINE|RELEASE_BASE_URL)\b/;

/** Words of a Bash line that may name a file: the guard resolves and checks each. */
export function pathWords(words: readonly string[]): string[] {
  return words.filter(
    (w) =>
      !w.startsWith("-") &&
      (w.includes("/") ||
        w.startsWith("~") ||
        STORE_FILE.test(w) ||
        w === "serve.json"),
  );
}

/** The guard's verdict on a Bash command's text alone, before its paths are resolved. */
export function bashDenied(command: string): string | null {
  if (GUARDED_VARIABLES.test(command)) {
    return "An agent never sets PLANROOM_PERSON_TOKEN, PLANROOM_RAW, PLANROOM_ENGINE or PLANROOM_RELEASE_BASE_URL.";
  }
  const words = shellWords(command);
  for (const line of planroomLines(words)) {
    const reason = personOnlyLine(line);
    if (reason !== null) return reason;
  }
  const push = words.findIndex(
    (w, i) => basename(w) === "git" && words[i + 1] === "push",
  );
  if (
    push >= 0 &&
    words.slice(push).some((w) => w.includes("refs/planroom/"))
  ) {
    return "Planroom's refs move only through the planroom CLI.";
  }
  return null;
}

/** The paths a call names, by tool, as the tool reads them. */
export function callPaths(
  tool: string,
  input: unknown,
): { reads: string[]; writes: string[]; bash: string | null } {
  const i = (
    typeof input === "object" && input !== null ? input : {}
  ) as Record<string, unknown>;
  const str = (k: string): string[] => {
    const v = i[k];
    return typeof v === "string" && v !== "" ? [v] : [];
  };
  switch (tool) {
    case "Read":
      return { reads: str("file_path"), writes: [], bash: null };
    case "Grep":
    case "Glob":
      return {
        reads: [
          ...str("path"),
          ...str("glob"),
          ...str("pattern").filter((p) => tool === "Glob" && p.includes("/")),
        ],
        writes: [],
        bash: null,
      };
    case "Edit":
    case "Write":
    case "MultiEdit":
      return { reads: [], writes: str("file_path"), bash: null };
    case "NotebookEdit":
      return { reads: [], writes: str("notebook_path"), bash: null };
    case "Bash":
      return { reads: [], writes: [], bash: str("command")[0] ?? "" };
    default:
      return { reads: [], writes: [], bash: null };
  }
}

/** Every path a call names, which the caller resolves before it asks `guardCall`; at most 32. */
export function namedPaths(tool: string, input: unknown): string[] {
  const { reads, writes, bash } = callPaths(tool, input);
  const fromBash = bash === null ? [] : pathWords(shellWords(bash));
  return [...new Set([...reads, ...writes, ...fromBash])].slice(0, 32);
}

/**
 * The verdict on one call, given every spelling of each path it names: its lexical form and,
 * where the file system resolved it, where it lands. Null lets the call go on to the session's
 * own permission rules.
 */
export function guardCall(
  tool: string,
  input: unknown,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
): Denial | null {
  const { reads, writes, bash } = callPaths(tool, input);
  const deny = (reason: string): Denial => ({ decision: "deny", reason });
  // The path as written, made absolute, then wherever the file system says it lands.
  const spell = (p: string): string[] => [
    normalize(p, places.cwd, places.home),
    ...resolved(p),
  ];
  for (const p of reads) {
    for (const s of spell(p)) {
      const reason = readDenied(s, places);
      if (reason !== null) return deny(reason);
      // A search rooted at a state directory or the data root reaches what's inside it.
      if (
        (tool === "Grep" || tool === "Glob") &&
        (inStateDir(s) || within(s, places.dataRoot))
      ) {
        return deny(
          "Planroom's state is read through the planroom CLI, never directly.",
        );
      }
    }
    if (tool !== "Read" && /store\.db|serve\.json/.test(p)) {
      return deny(
        "Planroom's store is read through the planroom CLI, never directly.",
      );
    }
  }
  for (const p of writes) {
    for (const s of spell(p)) {
      const reason = writeDenied(s, places);
      if (reason !== null) return deny(reason);
    }
  }
  if (bash !== null) {
    const reason = bashDenied(bash);
    if (reason !== null) return deny(reason);
    for (const p of pathWords(shellWords(bash))) {
      for (const s of spell(p)) {
        const why = readDenied(s, places);
        if (why !== null) return deny(why);
      }
    }
  }
  return null;
}
