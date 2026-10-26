/**
 * The mod's `tool.check` guard for every agent (architecture.md, "Actors"; DR-52, DR-64, DR-75):
 * no reads of the stores, the serve file, the data root's stores, ledger, backups and login pages,
 * the user-level config or the mod's own `$.store` file that holds the person key; no writes under
 * a state directory or the data root; and no Bash that runs a person-only command, sets a variable
 * that hands out the person's authority or picks the engine, or pushes `refs/planroom/`.
 *
 * Pure: register.tsx resolves each path (relative, `..`, symlinks) and passes every spelling. A
 * Bash line is read the way a shell would spell it out: quotes taken out, `$HOME` and the line's
 * own assignments put in, each path word from every directory a `cd` on the line may move to, and
 * a glob by every guarded place its literal prefix and segments may reach (security-3).
 *
 * It is a guardrail for "AI proposes, person disposes", not a boundary between processes of one
 * user: a script the agent runs by path, or writes and then runs, isn't read, nor is a variable set
 * in an earlier call or a glob that only `dotglob` lets reach a hidden folder.
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

/** Where the host keeps every plugin's `$.store` file (A-15, checked in WS8). */
function modStoreDir(places: GuardPlaces): string {
  return `${places.claudeConfig}/plugins/store`;
}

function basename(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** Whether a path is in a workspace's state directory: `.planroom/`, or a repo's `.git/planroom/`. */
function inStateDir(path: string): boolean {
  return /(^|\/)(\.planroom|\.git\/planroom)(\/|$)/.test(path);
}

const STORE_FILE = /^(store|ledger)\.db(-wal|-shm|-journal)?$/;

/** Another process's environment or memory, which may hold a person token (security-3). */
const PROC_SECRETS = /^\/proc\/[^/]+\/(task\/[^/]+\/)?(environ|mem)$/;

/** Why an agent may not read this path (absolute and normal), or null. */
export function readDenied(path: string, places: GuardPlaces): string | null {
  if (PROC_SECRETS.test(path)) {
    return "Another process's environment and memory may hold a person token.";
  }
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
  const store = modStoreDir(places);
  if (
    path === store ||
    (within(path, store) && /^planroom_/.test(basename(path)))
  ) {
    // A-15: the host keeps a plugin's `$.store` in `<config>/plugins/store/<plugin>_<source>-<hash>.json`.
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

/**
 * A shell command with its quotes and backslash escapes taken out, so a name split by quotes
 * (`PLANROOM_PERSON"_TOKEN"`, `plan'room'`) reads as the shell joins it.
 */
export function unquote(command: string): string {
  return command.replace(/\\\n/g, " ").replace(/["'\\]/g, "");
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

/** Glob metacharacters: a word holding one expands to whatever paths match it. */
const GLOB = /[*?[{]/;

/** A word that starts with an expansion: `$x`, `${x}`, `$(...)` or a backtick. */
function isExpansion(word: string): boolean {
  return word.startsWith("$") || word.startsWith("`");
}

/**
 * The simple commands of a shell line, quotes taken out, each as its words. Operators split it,
 * so `p=planroom; $p person init` is two commands.
 */
function simpleCommands(command: string): string[][] {
  return unquote(command)
    .split(/[;&|\n]+/)
    .map((c) => c.split(/[\s<>]+/).filter((w) => w !== ""));
}

/** The guard's verdict on a Bash command's text alone, before its paths are resolved. */
export function bashDenied(command: string): string | null {
  const plain = unquote(command);
  if (GUARDED_VARIABLES.test(plain)) {
    return "An agent never sets PLANROOM_PERSON_TOKEN, PLANROOM_RAW, PLANROOM_ENGINE or PLANROOM_RELEASE_BASE_URL.";
  }
  const words = shellWords(plain);
  for (const line of planroomLines(words)) {
    const reason = personOnlyLine(line);
    if (reason !== null) return reason;
  }
  // `$p person init`: an expansion in command position may be the engine, so the words after it
  // are read as a planroom line.
  for (const simple of simpleCommands(command)) {
    for (const [i, word] of simple.entries()) {
      if (!isExpansion(word)) continue;
      for (const line of planroomLines(["planroom", ...simple.slice(i + 1)])) {
        const reason = personOnlyLine(line);
        if (reason !== null) return reason;
      }
    }
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

/** The variables a path word may start from, as this session's environment set them. */
function knownVariables(places: GuardPlaces): Map<string, string> {
  const parent = (p: string): string =>
    p.slice(0, Math.max(1, p.lastIndexOf("/")));
  return new Map([
    ["HOME", places.home],
    ["PWD", places.cwd],
    ["CLAUDE_CONFIG_DIR", places.claudeConfig],
    ["PLANROOM_DATA", places.dataRoot],
    ["XDG_DATA_HOME", parent(places.dataRoot)],
    ["XDG_CONFIG_HOME", parent(parent(places.userConfig))],
  ]);
}

/**
 * `word` with each variable it names put in: a known one's value, one the line assigned before
 * its value, and any other expansion as `*`, since it could hold anything.
 */
function expand(word: string, vars: ReadonlyMap<string, string>): string {
  return word
    .replace(
      /\$\{([A-Za-z_][A-Za-z0-9_]*)[^}]*\}|\$([A-Za-z_][A-Za-z0-9_]*)/g,
      (_, braced: string | undefined, bare: string | undefined) =>
        vars.get(braced ?? bare ?? "") ?? "*",
    )
    .replace(/\$[^/]*|`[^/]*/g, "*");
}

/** The words of a Bash line that may name a file, its variables put in, and every directory it runs from. */
export interface BashPaths {
  words: string[];
  /** The session's directory, then each place a `cd` or `pushd` on the line may have moved to. */
  bases: string[];
}

/** Where a Bash line's words may land: each word that may name a file, and every `cd` target. */
export function bashPaths(command: string, places: GuardPlaces): BashPaths {
  const vars = knownVariables(places);
  const tokens = unquote(command)
    .split(/[\s;&|()<>`]+/)
    .filter((w) => w !== "");
  const candidates: string[] = [];
  let bases = [places.cwd];
  let moved = false;
  for (const [i, raw] of tokens.entries()) {
    // `NAME=value` and `--flag=value`: the value may be a path, and a name the line sets is known.
    const eq = raw.indexOf("=");
    const assigned =
      eq > 0 && /^[A-Za-z_][A-Za-z0-9_]*$/.test(raw.slice(0, eq));
    const value = eq >= 0 ? expand(raw.slice(eq + 1), vars) : null;
    if (assigned && value !== null) vars.set(raw.slice(0, eq), value);
    const word = expand(raw, vars);
    if (value !== null && value !== "") candidates.push(value);
    if (!assigned && !word.startsWith("-")) candidates.push(word);
    if (raw === "cd" || raw === "pushd") {
      const next = tokens[i + 1];
      const target =
        next === undefined || next.startsWith("-") || /^[;&|]/.test(next)
          ? "~"
          : expand(next, vars);
      const moves = bases.map((b) => normalize(target, b, places.home));
      bases = [...new Set([...bases, ...moves])].slice(0, 8);
      moved = true;
    }
  }
  const words = candidates.filter(
    (w) =>
      w !== "" &&
      (moved ||
        w.includes("/") ||
        w.startsWith("~") ||
        GLOB.test(w) ||
        STORE_FILE.test(w) ||
        w === "store.pointer" ||
        w === "serve.json"),
  );
  return { words: [...new Set(words)], bases };
}

/** A glob segment as a regular expression: `*` and `?` never match a leading dot, as in Bash. */
function segmentPattern(segment: string): string {
  let out = "";
  for (let i = 0; i < segment.length; i++) {
    const ch = segment[i] ?? "";
    if (ch === "*") out += "[^/]*";
    else if (ch === "?") out += "[^/]";
    else if (ch === "[") {
      const end = segment.indexOf("]", i + 2);
      if (end < 0) {
        out += "\\[";
        continue;
      }
      const set = segment.slice(i + 1, end).replace(/^!/, "^");
      out += `[${set.replace(/\\/g, "\\\\")}]`;
      i = end;
    } else if (ch === "{") {
      const end = segment.indexOf("}", i);
      if (end < 0) {
        out += "\\{";
        continue;
      }
      const alternatives = segment.slice(i + 1, end).split(",");
      out += `(?:${alternatives.map(segmentPattern).join("|")})`;
      i = end;
    } else out += ch.replace(/[.+^$()|\\\]}]/g, "\\$&");
  }
  return /^[*?[]/.test(segment) ? `(?!\\.)${out}` : out;
}

/** Whether the glob segment `segment` may match the name `name`. */
function segmentMatches(segment: string, name: string): boolean {
  if (segment === "**") return true;
  try {
    return new RegExp(`^${segmentPattern(segment)}$`).test(name);
  } catch {
    return true;
  }
}

/** The names Planroom keeps in a state directory, which an agent never reads. */
const STATE_SECRETS = [
  "store.db",
  "store.db-wal",
  "store.db-shm",
  "store.db-journal",
  "ledger.db",
  "store.pointer",
  "serve.json",
];

/**
 * Why a glob (absolute and normal) may expand into a place an agent may not read, or null: it
 * names one already; it lies inside a guarded directory; its literal prefix holds one that its
 * segments can reach; or it may name a store or serve file inside a state directory.
 */
export function globDenied(
  pattern: string,
  places: GuardPlaces,
): string | null {
  const direct = readDenied(pattern, places);
  if (direct !== null) return direct;
  const segments = pattern.split("/").filter((s) => s !== "");
  const first = segments.findIndex((s) => GLOB.test(s));
  if (first < 0) return null;
  const prefix = `/${segments.slice(0, first).join("/")}`;
  const rest = segments.slice(first);
  const data = places.dataRoot;
  const guarded = [
    ...["stores", "backups", "open", "ledger.db"].map((s) => `${data}/${s}`),
    places.userConfig,
    modStoreDir(places),
  ];
  for (const dir of guarded) {
    const why =
      readDenied(dir, places) ??
      "Planroom's private files are read through the planroom CLI, never directly.";
    if (within(prefix, dir)) return why;
    if (!within(dir, prefix)) continue;
    const below = dir
      .slice(prefix.length)
      .split("/")
      .filter((s) => s !== "");
    if (rest.includes("**")) return why;
    if (
      rest.length >= below.length &&
      below.every((name, i) => segmentMatches(rest[i] ?? "", name))
    ) {
      return why;
    }
  }
  // A store or serve file sits in a state directory: `.planroom/`, or a repo's `.git/planroom/`.
  const folders = segments.slice(0, -1);
  const last = segments.at(-1) ?? "";
  const stateFolder =
    inStateDir(prefix) ||
    folders.some(
      (s, i) =>
        segmentMatches(s, ".planroom") ||
        (segmentMatches(s, ".git") &&
          i + 1 < folders.length &&
          segmentMatches(folders[i + 1] ?? "", "planroom")),
    );
  if (stateFolder && STATE_SECRETS.some((n) => segmentMatches(last, n))) {
    return "Planroom's store is read through the planroom CLI, never directly.";
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
export function namedPaths(
  tool: string,
  input: unknown,
  places: GuardPlaces,
): string[] {
  const { reads, writes, bash } = callPaths(tool, input);
  const fromBash = bash === null ? [] : bashPaths(bash, places).words;
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
      const reason = GLOB.test(s)
        ? globDenied(s, places)
        : readDenied(s, places);
      if (reason !== null) return deny(reason);
      // A search rooted at a state directory, the data root, or a folder that holds the mod's
      // store reaches what's inside it.
      if (
        (tool === "Grep" || tool === "Glob") &&
        (inStateDir(s) ||
          within(s, places.dataRoot) ||
          within(modStoreDir(places), s))
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
    // Each word from every directory the line may run it in; a glob by what it may expand to.
    const { words, bases } = bashPaths(bash, places);
    for (const p of words) {
      const spellings = [
        ...bases.map((b) => normalize(p, b, places.home)),
        ...resolved(p),
      ];
      for (const s of spellings) {
        const why = GLOB.test(s)
          ? globDenied(s, places)
          : readDenied(s, places);
        if (why !== null) return deny(why);
      }
    }
  }
  return null;
}
