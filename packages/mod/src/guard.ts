/**
 * The mod's `tool.check` guard for every agent (architecture.md, "Actors"; DR-52, DR-64, DR-75):
 * no reads of the stores, the serve file, the data root's stores, ledger, backups and login pages,
 * the user-level config or the mod's own `$.store` file that holds the person key, by any tool and
 * any input that names a path (security-r3-2); no writes under a state directory, the data root or
 * the plugin's own folder (security-r3-3); and no Bash that runs a person-only command, sets a
 * variable that hands out the person's authority or picks the engine, or pushes `refs/planroom/`.
 *
 * Pure: register.tsx reads the shell's directory for each call (security-r3-1), resolves each path
 * (relative, `..`, symlinks) and passes every spelling. A
 * Bash line is read the way a shell would spell it out: quotes taken out, `$HOME` and the line's
 * own assignments put in, each path word from every directory a `cd` on the line may move to, and
 * a glob by every guarded place its literal prefix and segments may reach (security-3). Bash is
 * held to the Write tool's rule: a redirection's file and what `cp`, `mv`, `rm`, `tee`, `ln`,
 * `install`, `curl -o`, `sed -i` and the like write or delete are checked as writes, and a word in
 * the engine cache is denied whatever the command (security-r2-1). A command that reads, archives,
 * copies or links a whole tree (`grep -r`, `rg -uu`, `find -exec`, `tar`, `cp -r`, `rsync`,
 * `ln -s`) may not name a folder above a secret place, nor a state directory (security-r2-2).
 *
 * It is a guardrail for "AI proposes, person disposes", not a boundary between processes of one
 * user: a script the agent runs by path, or writes and then runs, isn't read, nor is a variable set
 * in an earlier call or a glob that only `dotglob` lets reach a hidden folder. Nor is a program
 * that writes or walks files through its own arguments (`python -c`, `node -e`, a build tool), a
 * tree read rooted above a workspace's `.planroom/` (`grep -r x .` reaches `serve.json`), or a
 * recursive copy of a crafted tree into a folder above the data root.
 */

import { CATALOGUE } from "@planroom/contract";

/** Where Planroom keeps what an agent mustn't read or write, for this user. */
export interface GuardPlaces {
  /**
   * The shell's directory for this call, for relative paths: the Bash tool keeps a `cd` between
   * calls, so register.tsx reads it per call (security-r3-1).
   */
  cwd: string;
  /** The workspace the session started in, whose state directory stays held wherever the shell is. */
  workspace: string;
  /**
   * The Planroom plugin's own folder, as the host names it and where it lands: it holds the mod
   * bundle with this guard and the person-key code, and the resolver every press runs (security-r3-3).
   */
  plugin: string[];
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
  more: { workspace?: string; plugin?: readonly string[] } = {},
): GuardPlaces {
  const home = env.HOME ?? "/";
  const nonEmpty = (v: string | undefined): string | undefined =>
    v === undefined || v === "" ? undefined : v;
  return {
    cwd,
    workspace: more.workspace ?? cwd,
    plugin: (more.plugin ?? []).filter((p) => p.startsWith("/")),
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

const STATE_WRITE =
  "Planroom's state directory changes only through the planroom CLI.";
const DATA_WRITE =
  "The Planroom data root changes only through the planroom CLI.";

const PLUGIN_WRITE =
  "The Planroom plugin's own files (the mod with its guard and person key code, and the resolver every press runs) change only when the person installs or updates the plugin.";

/** Why an agent may not write this path (absolute and normal), or null. */
export function writeDenied(path: string, places: GuardPlaces): string | null {
  if (inStateDir(path)) return STATE_WRITE;
  if (within(path, places.dataRoot)) return DATA_WRITE;
  if (places.plugin.some((p) => within(path, p))) return PLUGIN_WRITE;
  return readDenied(path, places);
}

/** Whether a path is in a state directory's `share/` folder, which `share` and `import` write. */
function inShareDir(path: string): boolean {
  return /(^|\/)(\.planroom|\.git\/planroom)\/share(\/|$)/.test(path);
}

/**
 * Why an agent may not send this file (absolute and normal) off the machine, through an Artifact,
 * its database or a file sent to the person, or null (security-r3-2). Beyond the read rule, only
 * the state directory's `share/` folder leaves it, since `planroom share` writes the Artifact's
 * bundle and `import` its batch files there for exactly that; the rest of the state directory and
 * the data root stay on this machine.
 */
function uploadDenied(path: string, places: GuardPlaces): string | null {
  const read = GLOB.test(path)
    ? globDenied(path, places)
    : readDenied(path, places);
  if (read !== null) return read;
  if (inStateDir(path) && !inShareDir(path)) {
    return "Of Planroom's state directory, only share/, which planroom share and import write for the Artifact, leaves this machine.";
  }
  if (within(path, places.dataRoot)) {
    return "The Planroom data root stays on this machine.";
  }
  return null;
}

/**
 * Why a `cd` or `pushd` may not move the shell into this folder, or null: a state directory, the
 * data root, the mod's store folder, the user config's folder or the plugin's own. The guard reads
 * the shell's directory for each call, so this holds only if a host ever doesn't report it
 * (security-r3-1).
 */
function cdDenied(path: string, places: GuardPlaces): string | null {
  const config = places.userConfig.slice(0, places.userConfig.lastIndexOf("/"));
  const held = [places.dataRoot, modStoreDir(places), config, ...places.plugin];
  return mayBeInStateDir(path) || held.some((h) => mayName(path, h, "within"))
    ? "An agent doesn't cd into Planroom's state, its data root, the mod's store, the user config's folder or the plugin's own folder; name the file from where you are, or use the planroom CLI."
    : null;
}

/**
 * The files an agent never reads, by any tool, nor reaches through a folder above them: the mod's
 * `$.store` folder (the person key), the user-level config (the gateway key), and the data root's
 * stores, backups, login pages and ledger (security-r2-2).
 */
function secretPlaces(places: GuardPlaces): string[] {
  const data = places.dataRoot;
  return [
    modStoreDir(places),
    places.userConfig,
    ...["stores", "backups", "open", "ledger.db"].map((s) => `${data}/${s}`),
  ];
}

/** The engine cache: every later session and every person press runs the binary in it. */
function engineDir(places: GuardPlaces): string {
  return `${places.dataRoot}/engine`;
}

function segmentsOf(path: string): string[] {
  return path.split("/").filter((s) => s !== "");
}

/** Whether one segment of a path, glob or not, may be the name `name`. */
function segmentMay(segment: string, name: string): boolean {
  return GLOB.test(segment) ? segmentMatches(segment, name) : segment === name;
}

/**
 * Whether `path` (absolute and normal, maybe a glob) may name `target` or something inside it
 * ("within"), or `target` or a folder above it ("above"). A `**` may reach anything.
 */
function mayName(
  path: string,
  target: string,
  mode: "within" | "above",
): boolean {
  const p = segmentsOf(path);
  const t = segmentsOf(target);
  for (let i = 0; i < Math.min(p.length, t.length); i++) {
    const segment = p[i] ?? "";
    if (segment === "**") return true;
    if (!segmentMay(segment, t[i] ?? "")) return false;
  }
  return mode === "within" ? p.length >= t.length : p.length <= t.length;
}

/** Whether a path, maybe a glob, may lie in a state directory: `.planroom/` or `.git/planroom/`. */
function mayBeInStateDir(path: string): boolean {
  const p = segmentsOf(path);
  return p.some(
    (s, i) =>
      segmentMay(s, ".planroom") ||
      (segmentMay(s, ".git") &&
        i + 1 < p.length &&
        segmentMay(p[i + 1] ?? "", "planroom")),
  );
}

/** Why an agent may not write, create or change what this path names, maybe a glob, or null. */
function writeTargetDenied(path: string, places: GuardPlaces): string | null {
  if (!GLOB.test(path)) return writeDenied(path, places);
  if (mayBeInStateDir(path)) return STATE_WRITE;
  if (mayName(path, places.dataRoot, "within")) return DATA_WRITE;
  if (places.plugin.some((p) => mayName(path, p, "within"))) {
    return PLUGIN_WRITE;
  }
  return globDenied(path, places);
}

/** Why an agent may not delete or move away this folder with all that's inside it, or null. */
function removeDenied(path: string, places: GuardPlaces): string | null {
  const held = [
    places.dataRoot,
    ...[places.workspace, places.cwd].flatMap((w) => [
      `${w}/.planroom`,
      `${w}/.git/planroom`,
    ]),
    modStoreDir(places),
    places.userConfig,
    ...places.plugin,
  ];
  return held.some((h) => mayName(path, h, "above"))
    ? "This deletes a folder that holds Planroom's state, its data root, the person key, the gateway key or the Planroom plugin."
    : null;
}

/**
 * Why an agent may not read, archive, copy or link this folder as a whole tree, or null: it holds
 * one of the secret places, or it's a state directory or the `.git` that may hold one. A tree read
 * that skips hidden names (`rg`, `fd`) reaches a secret place only along a path with none.
 */
function treeDenied(
  path: string,
  hidden: boolean,
  places: GuardPlaces,
): string | null {
  const reaches = (secret: string): boolean =>
    mayName(path, secret, "above") &&
    (hidden ||
      path.includes("**") ||
      !segmentsOf(secret)
        .slice(segmentsOf(path).length)
        .some((s) => s.startsWith(".")));
  const p = segmentsOf(path);
  const last = p.at(-1) ?? "";
  const state =
    segmentMay(last, ".planroom") ||
    segmentMay(last, ".git") ||
    (segmentMay(p.at(-2) ?? "", ".git") && segmentMay(last, "planroom"));
  return state || secretPlaces(places).some(reaches)
    ? "This reads, copies or links a whole folder that holds Planroom's private files: the person key, the gateway key, the stores, login pages or serve's token. Name a folder inside it that holds none of them."
    : null;
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
    // The planroom skill names the resolver through it; a shell that has it set has this plugin's.
    ...places.plugin
      .slice(0, 1)
      .map((p): [string, string] => ["CLAUDE_PLUGIN_ROOT", p]),
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

/** One simple command of a Bash line: its words, and the files its redirections name. */
interface SimpleCommand {
  words: string[];
  /** Files a `>`, `>>`, `>|`, `&>`, `>&FILE` or `<>` writes. */
  writes: string[];
  /** Files a `<` reads. */
  reads: string[];
}

/**
 * The simple commands of a Bash line, quotes taken out: operators, parentheses and backticks split
 * it, and each redirection's file is kept apart from the command's words. A here-document's
 * delimiter, a here-string and a duplicated descriptor (`2>&1`) name no file.
 */
function shellCommands(command: string): SimpleCommand[] {
  const text = unquote(command);
  const out: SimpleCommand[] = [];
  let cmd: SimpleCommand = { words: [], writes: [], reads: [] };
  let cur = "";
  let into: "word" | "write" | "read" | "skip" | "dup" = "word";
  const flush = (): void => {
    if (cur === "") return;
    if (into === "word") cmd.words.push(cur);
    else if (into === "write") cmd.writes.push(cur);
    else if (into === "read") cmd.reads.push(cur);
    else if (into === "dup" && !/^(\d+-?|-)$/.test(cur)) cmd.writes.push(cur);
    cur = "";
    into = "word";
  };
  const end = (): void => {
    flush();
    into = "word";
    if (cmd.words.length + cmd.writes.length + cmd.reads.length > 0) {
      out.push(cmd);
    }
    cmd = { words: [], writes: [], reads: [] };
  };
  // A redirection's operator: a descriptor number just before it belongs to it, not to the words.
  const operator = (): void => {
    if (/^\d+$/.test(cur)) cur = "";
    else flush();
  };
  for (let i = 0; i < text.length; i++) {
    const ch = text.charAt(i);
    const next = text.charAt(i + 1);
    if (";|()`\n".includes(ch)) {
      end();
    } else if (/\s/.test(ch)) {
      flush();
    } else if (ch === "&" && next === ">") {
      flush();
      i += text.charAt(i + 2) === ">" ? 2 : 1;
      into = "write";
    } else if (ch === "&") {
      end();
    } else if (ch === ">") {
      operator();
      if (next === ">" || next === "|") i++;
      if (text.charAt(i + 1) === "&") {
        i++;
        into = "dup";
      } else into = "write";
    } else if (ch === "<") {
      operator();
      if (next === "<") {
        while (text.charAt(i + 1) === "<" || text.charAt(i + 1) === "-") i++;
        into = "skip";
      } else if (next === ">") {
        i++;
        into = "write";
      } else if (next === "&") {
        i++;
        into = "skip";
      } else into = "read";
    } else cur += ch;
  }
  end();
  return out;
}

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

/** Commands that run the command after them, with each one's options that take a value. */
const WRAPPERS = new Map<string, readonly string[]>([
  ["sudo", ["-u", "-g", "-C", "-h", "-p", "-U", "-D", "-r", "-t"]],
  ["doas", ["-u", "-C"]],
  ["env", ["-u", "-C", "-S"]],
  ["xargs", ["-I", "-n", "-P", "-L", "-s", "-E", "-d", "-a"]],
  ["timeout", ["-s", "-k"]],
  ["nice", ["-n"]],
  ["command", []],
  ["builtin", []],
  ["exec", ["-a"]],
  ["nohup", []],
  ["time", []],
  ["stdbuf", []],
]);

/** Shell words that come before a command without being one. */
const KEYWORDS = new Set([
  "!",
  "{",
  "}",
  "if",
  "then",
  "else",
  "elif",
  "do",
  "while",
  "until",
]);

const SHELLS = new Set(["sh", "bash", "zsh", "dash", "ksh"]);

/** Where the command itself starts in a simple command's words: after assignments and wrappers. */
function commandStart(words: readonly string[]): number {
  let i = 0;
  while (i < words.length) {
    const word = words[i] ?? "";
    const name = basename(word);
    const valued = WRAPPERS.get(name);
    if (ASSIGNMENT.test(word) || KEYWORDS.has(word)) {
      i++;
    } else if (valued !== undefined) {
      i++;
      for (; i < words.length; i++) {
        const w = words[i] ?? "";
        if (valued.includes(w)) i++;
        else if (!w.startsWith("-") && !ASSIGNMENT.test(w)) break;
      }
      // timeout's duration
      if (name === "timeout" && /^\d/.test(words[i] ?? "")) i++;
    } else if (
      SHELLS.has(name) &&
      /^-[a-z]*c[a-z]*$/.test(words[i + 1] ?? "")
    ) {
      // `sh -c '...'`: its quotes are gone, so the words after `-c` are the command.
      i += 2;
    } else {
      return i;
    }
  }
  return i;
}

/** A tree a command reads, copies or links as a whole, and whether it reaches hidden names in it. */
export interface Tree {
  word: string;
  /** False for `rg`, `ag` and `fd` without a flag that includes hidden files: they skip dot names. */
  hidden: boolean;
}

/** What a Bash line's commands do to the paths they name, beyond reading them (security-r2-1, -2). */
export interface BashEffects {
  /** Files and folders a command may write, create, change or delete, redirections included. */
  writes: string[];
  /** Folders a command deletes or moves away whole, with all that's inside. */
  removes: string[];
  /** Folders a command reads, archives, copies or links as a whole tree. */
  trees: Tree[];
  /** Why the line is denied outright, whatever its paths, or null. */
  refusal: string | null;
}

const REMOVERS = new Set([
  "rm",
  "rmdir",
  "unlink",
  "shred",
  "trash",
  "trash-put",
]);
/** Commands whose every operand is a file they write or change. */
const WRITERS = new Set([
  "tee",
  "touch",
  "mkdir",
  "truncate",
  "chmod",
  "chown",
  "chgrp",
  "chattr",
  "setfacl",
  "mkfifo",
  "mknod",
  "tar",
]);
/** Commands that write their last operand, or the folder `-t` names. */
const COPIERS = new Set(["cp", "install", "ln", "rsync", "scp"]);
const IN_PLACE = new Set(["sed", "perl", "ruby"]);
const SEARCHERS = new Set(["grep", "egrep", "fgrep", "zgrep", "rgrep", "ack"]);
/** Searchers that skip hidden names unless a flag says otherwise. */
const SKIPPERS = new Set(["rg", "ag", "fd", "fdfind"]);
const FIND_ACTIONS = new Set(["-exec", "-execdir", "-ok", "-okdir"]);

/** What one simple command does to the paths it names, its words expanded. */
function commandEffects(all: readonly string[], piped: boolean): BashEffects {
  const effects: BashEffects = {
    writes: [],
    removes: [],
    trees: [],
    refusal: null,
  };
  const start = commandStart(all);
  const name = basename(all[start] ?? "");
  const args = all.slice(start + 1);
  const ops = args.filter((a) => !a.startsWith("-"));
  /** Whether a short-option cluster (`-rn`) holds one of these letters. */
  const short = (letters: string): boolean =>
    args.some(
      (a) =>
        /^-[^-]/.test(a) && [...a.slice(1)].some((c) => letters.includes(c)),
    );
  const long = (...names: string[]): boolean =>
    args.some((a) => names.some((n) => a === n || a.startsWith(`${n}=`)));
  /** The values of an option: `-o X`, `-oX`, `-sSo X`, `--output X` and `--output=X`. */
  const values = (letters: string, longs: readonly string[]): string[] => {
    const out: string[] = [];
    args.forEach((a, i) => {
      const after = args[i + 1];
      for (const n of longs) {
        if (a === n && after !== undefined) out.push(after);
        else if (a.startsWith(`${n}=`)) out.push(a.slice(n.length + 1));
      }
      if (!/^-[A-Za-z]/.test(a)) return;
      const at = [...a.slice(1)].findIndex((c) => letters.includes(c));
      if (at < 0) return;
      const rest = a.slice(at + 2);
      if (rest !== "") out.push(rest);
      else if (after !== undefined) out.push(after);
    });
    return out;
  };
  const trees = (words: readonly string[], hidden = true): void => {
    effects.trees.push(...words.map((word) => ({ word, hidden })));
  };
  /** A search's roots: its operands after the pattern, or `.` when it names none. */
  const roots = (): string[] => {
    const given =
      values("ef", ["--regexp", "--file"]).length > 0 || long("--files");
    const named = given ? ops : ops.slice(1);
    return named.length > 0 ? named : ["."];
  };
  const target = values("t", ["--target-directory"]);
  if (REMOVERS.has(name)) {
    effects.writes.push(...ops);
    effects.removes.push(...ops);
  } else if (name === "mv") {
    effects.writes.push(...ops);
    effects.removes.push(
      ...(target.length > 0
        ? ops.filter((o) => !target.includes(o))
        : ops.slice(0, -1)),
    );
  } else if (WRITERS.has(name)) {
    effects.writes.push(...ops);
    if (name === "tar") trees(ops);
  } else if (COPIERS.has(name)) {
    const last = ops.length >= 2 ? ops.slice(-1) : [];
    effects.writes.push(
      ...target,
      ...(name === "install" && short("d") ? ops : last),
    );
    if (
      name === "rsync" ||
      (name === "cp" && (short("rRa") || long("--recursive", "--archive"))) ||
      (name === "scp" && short("r")) ||
      (name === "ln" && (short("s") || long("--symbolic")))
    ) {
      trees(ops);
    }
  } else if (IN_PLACE.has(name)) {
    if (short("i") || long("--in-place")) effects.writes.push(...ops);
  } else if (name === "dd") {
    effects.writes.push(
      ...args.filter((a) => a.startsWith("of=")).map((a) => a.slice(3)),
    );
  } else if (name === "curl") {
    effects.writes.push(...values("o", ["--output", "--output-dir"]));
  } else if (name === "wget") {
    effects.writes.push(
      ...values("OP", ["--output-document", "--directory-prefix"]),
    );
  } else if (name === "unzip") {
    effects.writes.push(...values("d", []));
  } else if (name === "zip") {
    effects.writes.push(...ops.slice(0, 1));
    if (short("rR") || long("--recurse-paths")) trees(ops);
  } else if (name === "find") {
    const expression = args.findIndex((a) => a.startsWith("-") || a === "!");
    const paths = expression < 0 ? args : args.slice(0, expression);
    const from = paths.length > 0 ? paths : ["."];
    if (piped || args.some((a) => FIND_ACTIONS.has(a))) trees(from);
    if (args.includes("-delete")) effects.writes.push(...from);
  } else if (SEARCHERS.has(name)) {
    const recursive =
      name === "rgrep" ||
      name === "ack" ||
      short("rR") ||
      long("--recursive", "--dereference-recursive") ||
      args.includes("--directories=recurse") ||
      args.some((a, i) => a === "-d" && args[i + 1] === "recurse");
    if (recursive) trees(roots());
  } else if (SKIPPERS.has(name)) {
    const fd = name === "fd" || name === "fdfind";
    const hidden =
      long("--hidden", "--unrestricted") ||
      args.some((a) => a.startsWith("--no-ignore")) ||
      short(fd ? "uHI" : "u.");
    trees(roots(), hidden);
  } else if (name === "git" && ops.includes("clean")) {
    // `.planroom/` ignores itself, so `git clean -x` deletes it, the store with it, unless `-e`
    // keeps it; `-X` deletes only ignored files, `-e` patterns among them.
    const kept =
      !short("X") &&
      values("e", ["--exclude"]).some((e) => /^\/?\.planroom\/?$/.test(e));
    if (short("d") && short("xX") && !kept) {
      effects.refusal =
        "git clean -x deletes the ignored .planroom/, Planroom's store with it: leave out -x, or add -e .planroom.";
    }
  }
  return effects;
}

/** The words of a Bash line that may name a file, its variables put in, and every directory it runs from. */
export interface BashPaths {
  words: string[];
  /** The words of `words` a command takes as an argument, not the program it runs. */
  args: string[];
  /** The shell's directory, then each place a `cd` or `pushd` on the line may have moved to. */
  bases: string[];
  /** Where each `cd` or `pushd` on the line goes, as written, its variables put in. */
  cds: string[];
  effects: BashEffects;
}

/**
 * Where a Bash line's words may land: each word that may name a file, every `cd` target, and what
 * its commands write, delete and read as whole trees. Every word an effect names is also a word,
 * so the caller resolves it.
 */
export function bashPaths(command: string, places: GuardPlaces): BashPaths {
  const vars = knownVariables(places);
  const commands = shellCommands(command);
  const piped = commands.some((c) =>
    c.words.some((w) => basename(w) === "xargs"),
  );
  const candidates: string[] = [];
  const effects: BashEffects = {
    writes: [],
    removes: [],
    trees: [],
    refusal: null,
  };
  let bases = [places.cwd];
  const cds: string[] = [];
  // A shell outside the workspace's root, left there by an earlier call's `cd`, may sit where a
  // bare name is a guarded file (`cat config.toml`), so every word counts as a path (security-r3-1).
  let moved = places.cwd !== places.workspace;
  for (const simple of commands) {
    const words: string[] = [];
    const values: (string | null)[] = [];
    const assignments: boolean[] = [];
    for (const [i, raw] of simple.words.entries()) {
      // `NAME=value` and `--flag=value`: the value may be a path, and a name the line sets is known.
      const eq = raw.indexOf("=");
      const assigned =
        eq > 0 && /^[A-Za-z_][A-Za-z0-9_]*$/.test(raw.slice(0, eq));
      const value = eq >= 0 ? expand(raw.slice(eq + 1), vars) : null;
      if (assigned && value !== null) vars.set(raw.slice(0, eq), value);
      words.push(expand(raw, vars));
      values.push(value);
      assignments.push(assigned);
      if (raw === "cd" || raw === "pushd") {
        const next = simple.words[i + 1];
        const target =
          next === undefined || next.startsWith("-") ? "~" : expand(next, vars);
        cds.push(target);
        const moves = bases.map((b) => normalize(target, b, places.home));
        bases = [...new Set([...bases, ...moves])].slice(0, 8);
        moved = true;
      }
    }
    words.forEach((word, i) => {
      const value = values[i] ?? null;
      if (value !== null && value !== "") candidates.push(value);
      if (assignments[i] !== true && !word.startsWith("-")) {
        candidates.push(word);
      }
    });
    const writes = simple.writes.map((w) => expand(w, vars));
    candidates.push(...writes, ...simple.reads.map((w) => expand(w, vars)));
    const own = commandEffects(words, piped);
    effects.writes.push(...writes, ...own.writes);
    effects.removes.push(...own.removes);
    effects.trees.push(...own.trees);
    effects.refusal ??= own.refusal;
  }
  const named = candidates.filter(
    (w) =>
      moved ||
      w.includes("/") ||
      w.startsWith("~") ||
      GLOB.test(w) ||
      STORE_FILE.test(w) ||
      w === "store.pointer" ||
      w === "serve.json",
  );
  const operands = [
    ...effects.writes,
    ...effects.removes,
    ...effects.trees.map((t) => t.word),
  ];
  const words = [...named, ...operands].filter((w) => w !== "");
  // The programs the line runs, one each per command the shell's operators split it into (not its
  // parentheses, which may be quoted text, as in `python3 -c 'open(1)' FILE`). Each takes one
  // occurrence of its word out of the arguments, so a word named again as an argument stays one.
  const known = knownVariables(places);
  const runs = simpleCommands(command).map((c) => {
    const expanded = c.map((w) => expand(w, known));
    return expanded[commandStart(expanded)] ?? "";
  });
  const args = [...named, ...operands].filter((w) => {
    const at = runs.indexOf(w);
    if (at < 0) return w !== "";
    runs.splice(at, 1);
    return false;
  });
  return {
    words: [...new Set(words)],
    args: [...new Set(args)],
    bases,
    cds,
    effects,
  };
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

/** The paths a call names, by what the tool does with each. */
export interface CallPaths {
  /** Files and folders the tool reads. */
  reads: string[];
  /** Files and folders it writes or creates. */
  writes: string[];
  /** Files it sends off this machine: to an Artifact, its database, or the person's device. */
  uploads: string[];
  /** A shell command it runs, or null. */
  bash: string | null;
}

/** A string an unknown tool may take as a file: a path to a folder or file, never a URL or prose. */
function pathLike(value: string): boolean {
  if (value === "" || value.includes("\n") || value.includes("://")) {
    return false;
  }
  if (/^(\/|~|\.\.?\/)/.test(value)) return true;
  if (/\s/.test(value)) return false;
  return (
    value.includes("/") ||
    STORE_FILE.test(value) ||
    value === "store.pointer" ||
    value === "serve.json"
  );
}

/** Every string an input holds, at any depth, keys left out; at most 64 of them. */
function stringsIn(value: unknown, depth = 0, out: string[] = []): string[] {
  if (out.length >= 64 || depth > 8) return out;
  if (typeof value === "string") out.push(value);
  else if (Array.isArray(value)) {
    for (const v of value) stringsIn(v, depth + 1, out);
  } else if (typeof value === "object" && value !== null) {
    for (const v of Object.values(value)) stringsIn(v, depth + 1, out);
  }
  return out;
}

/**
 * The paths a call names, by tool, as the tool takes them (security-r3-2). Every tool's
 * path-bearing inputs count, not a fixed few: a tool the guard doesn't know has each string it
 * holds that looks like a path held to the read rule.
 */
export function callPaths(tool: string, input: unknown): CallPaths {
  const i = (
    typeof input === "object" && input !== null ? input : {}
  ) as Record<string, unknown>;
  const str = (k: string, from: Record<string, unknown> = i): string[] => {
    const v = from[k];
    return typeof v === "string" && v !== "" ? [v] : [];
  };
  const strs = (k: string): string[] => {
    const v = i[k];
    return Array.isArray(v)
      ? v.filter((s): s is string => typeof s === "string" && s !== "")
      : str(k);
  };
  const records = (k: string): Record<string, unknown>[] => {
    const v = i[k];
    return Array.isArray(v)
      ? v.filter(
          (r): r is Record<string, unknown> =>
            typeof r === "object" && r !== null,
        )
      : [];
  };
  const none: CallPaths = { reads: [], writes: [], uploads: [], bash: null };
  switch (tool) {
    case "Read":
      return { ...none, reads: str("file_path") };
    case "Grep":
    case "Glob":
      return {
        ...none,
        reads: [
          ...str("path"),
          ...str("glob"),
          ...str("pattern").filter((p) => tool === "Glob" && p.includes("/")),
        ],
      };
    case "LS":
      return { ...none, reads: str("path") };
    case "NotebookRead":
      return { ...none, reads: [...str("notebook_path"), ...str("path")] };
    case "LSP":
      return { ...none, reads: str("filePath") };
    case "Edit":
    case "Write":
    case "MultiEdit":
      return { ...none, writes: str("file_path") };
    case "NotebookEdit":
      return { ...none, writes: str("notebook_path") };
    case "Bash":
      return { ...none, bash: str("command")[0] ?? "" };
    case "Monitor": {
      // A Monitor's command is a shell command like Bash's; one that watches a socket names none.
      const command = str("command")[0];
      return { ...none, bash: command ?? null };
    }
    case "Artifact": {
      // Each `files` source is relative to `root` when one is given; a source in another
      // Artifact ({artifact, path}) isn't a local file.
      const root = str("root")[0];
      const under = (p: string): string =>
        root === undefined || /^(\/|~)/.test(p) ? p : `${root}/${p}`;
      const files = i["files"];
      const sources: string[] = [];
      const source = (v: unknown): void => {
        if (typeof v === "string" && v !== "") sources.push(under(v));
        else if (typeof v === "object" && v !== null && !("artifact" in v)) {
          const from = (v as Record<string, unknown>)["from"];
          const path = (v as Record<string, unknown>)["path"];
          if (typeof from === "string" && from !== "")
            sources.push(under(from));
          else if (typeof path === "string" && path !== "") {
            sources.push(under(path));
          }
        }
      };
      if (Array.isArray(files)) files.forEach(source);
      else if (typeof files === "object" && files !== null) {
        Object.values(files).forEach(source);
      }
      return {
        ...none,
        uploads: [
          ...str("file_path"),
          ...strs("file_paths"),
          ...sources,
          ...str("root"),
        ],
        writes: str("out_dir"),
      };
    }
    case "ArtifactData":
      // `file_path` is a local JSON file sent as the document; so is each batch entry's.
      return {
        ...none,
        uploads: [
          ...str("file_path"),
          ...[...records("writes"), ...records("batch")].flatMap((w) =>
            str("file_path", w),
          ),
        ],
        writes: str("out_dir"),
      };
    case "SendUserFile":
    case "SendFile":
      return { ...none, uploads: strs("files") };
    default:
      return { ...none, reads: stringsIn(input).filter(pathLike) };
  }
}

/** Every path a call names, which the caller resolves before it asks `guardCall`; at most 64. */
export function namedPaths(
  tool: string,
  input: unknown,
  places: GuardPlaces,
): string[] {
  const { reads, writes, uploads, bash } = callPaths(tool, input);
  const fromBash = bash === null ? [] : bashPaths(bash, places).words;
  return [...new Set([...reads, ...writes, ...uploads, ...fromBash])].slice(
    0,
    64,
  );
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
  const { reads, writes, uploads, bash } = callPaths(tool, input);
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
      // store, the user-level config or the data root's private files reaches what's inside it.
      if (
        (tool === "Grep" || tool === "Glob") &&
        (inStateDir(s) ||
          within(s, places.dataRoot) ||
          secretPlaces(places).some((secret) => within(secret, s)))
      ) {
        return deny(
          "Planroom's state is read through the planroom CLI, never directly.",
        );
      }
    }
    if (
      (tool === "Grep" || tool === "Glob") &&
      /store\.db|serve\.json/.test(p)
    ) {
      return deny(
        "Planroom's store is read through the planroom CLI, never directly.",
      );
    }
  }
  for (const p of uploads) {
    for (const s of spell(p)) {
      const reason = uploadDenied(s, places);
      if (reason !== null) return deny(reason);
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
    const { words, args, bases, cds, effects } = bashPaths(bash, places);
    if (effects.refusal !== null) return deny(effects.refusal);
    const spellings = (p: string): string[] => [
      ...bases.map((b) => normalize(p, b, places.home)),
      ...resolved(p),
    ];
    const first = (
      paths: readonly string[],
      why: (s: string) => string | null,
    ): string | null => {
      for (const p of paths) {
        for (const s of spellings(p)) {
          const reason = why(s);
          if (reason !== null) return reason;
        }
      }
      return null;
    };
    const why =
      first(words, (s) =>
        mayName(s, engineDir(places), "within")
          ? "Planroom's engine cache runs with the person's authority: only the resolver writes it, and an agent's command never names it."
          : GLOB.test(s)
            ? globDenied(s, places)
            : readDenied(s, places),
      ) ??
      // security-r3-3: the plugin's folder is named only to run what's in it, such as the resolver
      // the planroom skill has the agent run; as any other word, a program may write it.
      first(args, (s) =>
        places.plugin.some((p) => mayName(s, p, "within"))
          ? PLUGIN_WRITE
          : null,
      ) ??
      // security-r3-1: a host reports where the shell is, but a cd into a guarded place is refused
      // too, so a host that didn't still leaves no relative path there.
      first(cds, (s) => cdDenied(s, places)) ??
      // security-r2-1: what the Write tool may not write, no command or redirection writes either.
      first(effects.writes, (s) => writeTargetDenied(s, places)) ??
      first(effects.removes, (s) => removeDenied(s, places)) ??
      // security-r2-2: a whole-tree read reaches whatever its folder holds.
      effects.trees
        .map((t) => first([t.word], (s) => treeDenied(s, t.hidden, places)))
        .find((r) => r !== null) ??
      null;
    if (why !== null) return deny(why);
  }
  return null;
}
