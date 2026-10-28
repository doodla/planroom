/**
 * The mod's `tool.check` guard for every agent (architecture.md, "Actors"; DR-52, DR-64, DR-75):
 * no reads of the stores, the serve file, the data root's stores, ledger, backups, login pages and
 * serve records (each with its control token), the user-level config or the mod's own `$.store`
 * file that holds the person key, by any tool and any input that names a path (security-r3-2); no writes under a state directory, the data root or
 * the plugin's own folder (security-r3-3); and no Bash that runs a person-only command, sets a
 * variable that hands out the person's authority or picks the engine, or pushes `refs/planroom/`,
 * nor any write that puts such a variable in a shell's start-up file or a Claude Code settings
 * file. A call it would let through that runs on another attached machine (`_host`) or outside the
 * sandbox (`dangerouslyDisableSandbox`) is put to the person.
 *
 * Pure: register.tsx reads the shell's directory for each call (security-r3-1), resolves each path
 * (relative, `..`, symlinks) and passes every spelling, or asks past `RESOLVE_LIMIT` paths. With no
 * absolute HOME it can't place what it guards, so it asks before every call (tests-3-1).
 *
 * A Bash line is read the way a shell would spell it out: quotes taken out, `$HOME`, `~name` and
 * the line's own assignments put in, each path word from every directory a `cd` (or `env -C`,
 * `sudo -D`) on the line may move to, and a glob by every guarded place its literal prefix and
 * segments may reach, `**` as any number of folders, and hidden names only when the line turns
 * dotglob on (security-3). It's read other ways too: with quoted or escaped operators as text and
 * each command substitution as an unknown word, with each `${X:-word}` as its `word`, and with
 * every backslash-newline a quoted `sh -c` script would drop dropped; the guard denies what any
 * reading denies. Bash is held to the Write tool's rule: a redirection's file
 * and what `cp`, `mv`, `rm`, `tee`, `ln`, `install`, `curl -o`, `sed -i` and the like write or
 * delete are checked as writes, and a word in the engine cache is denied whatever the command
 * (security-r2-1). A command that reads, archives, copies or links a whole tree (`grep -r`,
 * `git grep --no-index`, `rg -uu`, `find -exec`, `tar`, `7z`, `cp -r`, `rsync`, `ln -s`) may not
 * name a folder above a secret place, nor a state directory (security-r2-2). An archive may not
 * unpack, nor `mv` move a crafted tree, above the data root, the mod's store, the user config or
 * the plugin; a deleting `find`, or an `rm` that xargs or a command substitution feeds, may not
 * select Planroom's state (security-r3-4).
 *
 * A spawned implementer's calls (`GuardPlaces.implementer`) write with no prompt only in its
 * hand-off's clones, outside each clone's `.git/`, and its `result.json`, read its clones under the
 * session's own rules, and run no Bash in M1 (DR-52, DR-64, DR-75).
 *
 * It is a guardrail for "AI proposes, person disposes", not a boundary between processes of one
 * user, and architecture.md ("Actors") lists what it doesn't read: a script the agent runs by
 * path, a variable or `dotglob` set outside the line, a program that walks or writes files through
 * its own arguments (`python -c`, `node -e`, a build tool), a tree read rooted at or above the
 * workspace (`grep -r x .` reaches the stores in its state directory, though no control token), a
 * crafted tree unpacked into the workspace, and deleting a folder that holds another workspace.
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
  /** `$ZDOTDIR`, where zsh reads its start-up files, when the environment sets an absolute one. */
  zdotdir?: string;
  /**
   * The git directories that a `.git` file names at the workspace's root and at the shell's
   * directory, a linked worktree's or a submodule's, each absolute and normal: the state directory
   * is `planroom/` inside it (DR-74). register.tsx reads them; without them, the state directory's
   * path patterns still hold.
   */
  gitDirs?: readonly string[];
  /**
   * Why the guard can't place what it guards, when the environment gives no absolute `HOME`: it
   * then asks before every call rather than check against the wrong folders (tests-3-1).
   */
  unplaced?: string;
  /**
   * Set when the call is a spawned `planroom:implementer`'s [A-12]: its hand-off's clones under
   * `DATA_ROOT/handoffs/`, each absolute and normal, and the hand-off's `result.json`. It writes
   * only there, outside each clone's `.git/`, and runs no Bash in M1 (DR-52, DR-64, DR-75).
   */
  implementer?: { clones: readonly string[]; result: string };
}

/** The guard's verdict on a call it doesn't let through to the session's own rules. */
export interface Verdict {
  /**
   * `deny` refuses the call; `ask` puts it to the person, and where nobody can be asked it's a
   * deny; `allow` lets it run with no prompt, as the implementer's writes in its own clones do.
   */
  decision: "deny" | "ask" | "allow";
  reason: string;
}

/** The name the mod's wiring uses for a verdict. */
export type Denial = Verdict;

/** An absolute path from the environment, or undefined: unset, empty and relative values don't count. */
function absolute(value: string | undefined): string | undefined {
  return value?.startsWith("/") === true ? value : undefined;
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
    ZDOTDIR?: string | undefined;
  },
  more: { workspace?: string; plugin?: readonly string[] } = {},
): GuardPlaces {
  const home = absolute(env.HOME);
  const h = home === undefined ? "/" : normalize(home, "/", "/");
  // Planroom's own variables, as a process started in the session's directory reads them; the XDG
  // spec says to ignore a relative XDG_DATA_HOME or XDG_CONFIG_HOME, and the engine does.
  const named = (v: string | undefined): string | undefined =>
    v === undefined || v === "" ? undefined : normalize(v, cwd, h);
  const xdg = (v: string | undefined): string | undefined => {
    const a = absolute(v);
    return a === undefined ? undefined : normalize(a, "/", h);
  };
  const zdotdir = xdg(env.ZDOTDIR);
  return {
    cwd,
    workspace: more.workspace ?? cwd,
    plugin: (more.plugin ?? []).filter((p) => p.startsWith("/")),
    home: h,
    claudeConfig: named(env.CLAUDE_CONFIG_DIR) ?? normalize(".claude", h, h),
    dataRoot:
      named(env.PLANROOM_DATA) ??
      normalize("planroom", xdg(env.XDG_DATA_HOME) ?? `${h}/.local/share`, h),
    userConfig: normalize(
      "planroom/config.toml",
      xdg(env.XDG_CONFIG_HOME) ?? `${h}/.config`,
      h,
    ),
    ...(zdotdir === undefined ? {} : { zdotdir }),
    ...(home === undefined
      ? {
          unplaced:
            "The Planroom guard can't tell where your home folder is (HOME is unset or not absolute), so it can't check what this call reaches.",
        }
      : {}),
  };
}

/**
 * The data root as the resolver (plugin/scripts/planroom) places it from this environment, for the
 * mod to pin on every run it starts, so a value that changes later, such as a settings file's
 * `env`, can't move the engine cache a press runs: `PLANROOM_DATA` as given (a relative one stays
 * relative, and the resolver refuses it), else an absolute `XDG_DATA_HOME`'s, else an absolute
 * `HOME`'s. When none gives one, the relative `unplaced`, which the resolver refuses as it refuses
 * no place at all, rather than follow a value the environment takes on later.
 */
export function pinnedDataRoot(env: {
  HOME?: string | undefined;
  PLANROOM_DATA?: string | undefined;
  XDG_DATA_HOME?: string | undefined;
}): string {
  if (env.PLANROOM_DATA !== undefined && env.PLANROOM_DATA !== "") {
    return env.PLANROOM_DATA;
  }
  const home = absolute(env.HOME);
  const base =
    absolute(env.XDG_DATA_HOME) ??
    (home === undefined ? undefined : `${home}/.local/share`);
  return base === undefined ? UNPLACED : `${base}/planroom`;
}

/** The data root pinned when the environment gives none: relative, so the resolver refuses it. */
export const UNPLACED = "unplaced";

/**
 * An absolute, lexically normal path: `~`, `~+` and `~name` expanded, relative to `cwd`, `.` and
 * `..` folded. `~name` is read as this user's home, the one whose secrets the guard holds
 * (security-r2-4), and `~-`, the shell's last directory, which the guard can't know, as home too.
 */
export function normalize(path: string, cwd: string, home: string): string {
  let p = path.trim();
  const tilde = /^~([^/]*)(\/|$)/.exec(p);
  if (tilde !== null) {
    p = `${tilde[1] === "+" ? cwd : home}/${p.slice(tilde[0].length)}`;
  }
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

/**
 * The entries git keeps in a git directory (gitrepository-layout(5)), which a later segment of a
 * submodule's name is read never to be: so a branch named planroom
 * (`.git/modules/lib/refs/heads/planroom`) isn't state, and `rm -f .git/modules/lib/index.lock`
 * deletes no folder above any.
 */
const GIT_DIR_ENTRY = new RegExp(
  "^(" +
    [
      "refs|logs|objects|hooks|info|branches|remotes|lfs|rr-cache",
      "rebase-merge|rebase-apply|sequencer",
      "index|packed-refs|config|config\\.worktree|description|shallow",
      "commondir|gitdir|locked|gc\\.log|gc\\.pid|sharedindex\\..*",
      "HEAD|ORIG_HEAD|FETCH_HEAD|MERGE_HEAD|MERGE_MSG|MERGE_MODE|MERGE_RR",
      "CHERRY_PICK_HEAD|REVERT_HEAD|AUTO_MERGE|COMMIT_EDITMSG|SQUASH_MSG",
      "TAG_EDITMSG|BISECT_.*|.*\\.lock",
    ].join("|") +
    ")$",
);

/**
 * The indexes of the segments that may each end a state directory (DR-74): `.planroom`, or
 * `planroom` directly inside a git directory, as `git rev-parse --absolute-git-dir` names it. That
 * is a repo's `.git`, a linked worktree's `.git/worktrees/<name>`, or a submodule's
 * `.git/modules/<name>`, whose name may take several segments, at any depth of `modules/` and with
 * its own `worktrees/`. `may` says whether a segment, maybe a glob, may be a name.
 *
 * A `**` stands for any number of folders, so wherever it stands in a git directory, for a
 * worktree's name or a submodule's included, it may reach a `planroom/` below, as `.git/**` does:
 * `.git/worktrees/**` and `.git/modules/**` both may. A `**` matches no hidden folder, so it never
 * reaches a `.git` it doesn't spell; a git directory under no hidden folder, such as a bare repo's,
 * is held through the `.git` file that names it (`namedGitDirs`). A `*` or `?` stands for one name,
 * as a literal name does.
 */
function stateDirEnds(
  p: readonly string[],
  may: (segment: string, name: string) => boolean,
): number[] {
  const ends = new Set<number>();
  p.forEach((s, i) => {
    if (may(s, ".planroom")) ends.add(i);
  });
  for (let g = 0; g < p.length; g++) {
    if (!may(p[g] ?? "", ".git")) continue;
    // Each index where a git directory's own entries start, each walked once.
    const seen = new Set<number>();
    const todo = [g + 1];
    for (let k = todo.pop(); k !== undefined; k = todo.pop()) {
      if (k >= p.length || seen.has(k)) continue;
      seen.add(k);
      const s = p[k] ?? "";
      if (may(s, "planroom")) ends.add(k);
      // A `**` here may stand for no folder, or for the path down to another git directory.
      if (isGlobstar(s)) todo.push(k + 1);
      const named = may(s, "worktrees") || may(s, "modules");
      if (may(s, "worktrees")) todo.push(k + 2);
      if (may(s, "modules")) {
        // The name's first segment may be anything; it goes on until an entry git keeps.
        for (let n = k + 2; n <= p.length; n++) {
          todo.push(n);
          if (GIT_DIR_ENTRY.test(p[n] ?? "")) break;
        }
      }
      // A `**` for the name may stand for the name and what's below it in its git directory.
      if (named && isGlobstar(p[k + 1] ?? "")) todo.push(k + 1);
    }
  }
  return [...ends];
}

/** Whether two names are one, for a path the guard reads as written, with no glob in it. */
function same(segment: string, name: string): boolean {
  return segment === name;
}

/**
 * The git directories a `.git` file names, each with the state directory in it (DR-74), wherever
 * its path lies: a separate git directory such as `/srv/git/app`, or a submodule's whose name the
 * path patterns stop reading early, such as `.git/modules/app/config`.
 */
function namedGitDirs(
  places: GuardPlaces,
): { gitDir: string; state: string }[] {
  return (places.gitDirs ?? []).map((g) => {
    const gitDir = g.replace(/\/+$/, "");
    return { gitDir, state: `${gitDir}/planroom` };
  });
}

/**
 * Whether a path is in a workspace's state directory: `.planroom/`, or `planroom/` in a repo's,
 * a linked worktree's or a submodule's git directory, or in the git directory a `.git` file names.
 */
function inStateDir(path: string, places: GuardPlaces): boolean {
  return (
    stateDirEnds(segmentsOf(path), same).length > 0 ||
    namedGitDirs(places).some(({ state }) => within(path, state))
  );
}

/**
 * Whether a path, maybe a glob, may name a state directory, or a folder above one in a git
 * directory from the `.git` down: `.git`, `.git/planroom`, `.git/worktrees`, `.git/worktrees/wt`,
 * `.git/modules/lib`, and a git directory a `.git` file names. Not `.git/refs`, nor a folder
 * inside a state directory.
 */
function mayHoldState(path: string, places: GuardPlaces): boolean {
  const p = segmentsOf(path);
  return (
    [["planroom"], ["wt", "planroom"]].some((more) =>
      stateDirEnds([...p, ...more], segmentMay).some((k) => k >= p.length - 1),
    ) ||
    namedGitDirs(places).some(
      ({ gitDir, state }) =>
        mayName(path, gitDir, "within") && mayName(path, state, "above"),
    )
  );
}

/**
 * The git directory a `.git` file names, read as git reads it, or null: a linked worktree's or a
 * submodule's `.git` is a file whose `gitdir: ` line gives the path, relative to the folder the
 * file is in unless absolute. `folder` is absolute and normal.
 */
export function gitFileDir(text: string, folder: string): string | null {
  const body = text.replace(/[\r\n]+$/, "");
  if (!body.startsWith("gitdir: ")) return null;
  const dir = body.slice("gitdir: ".length);
  if (dir === "" || dir.trim() !== dir || /[\r\n]/.test(dir)) return null;
  return normalize(dir.startsWith("/") ? dir : `${folder}/${dir}`, "/", "/");
}

const STORE_FILE = /^(store|ledger)\.db(-wal|-shm|-journal)?$/;

/**
 * What no agent reads in the data root: the stores, backups, login pages, each running `serve`'s
 * record with its control token (`serve/<port>.json`), and the ledger.
 */
const DATA_SECRETS = ["stores", "backups", "open", "serve", "ledger.db"];

/** Another process's environment or memory, which may hold a person token (security-3). */
const PROC_SECRETS = /^\/proc\/[^/]+\/(task\/[^/]+\/)?(environ|mem)$/;

/**
 * Whether a folder on the way to this path is a glob, or an expansion the guard reads as one
 * (`$X/store.db`), which may stand for a state directory.
 */
function globbedFolder(path: string): boolean {
  return segmentsOf(path)
    .slice(0, -1)
    .some((s) => GLOB.test(s));
}

/** Why an agent may not read this path (absolute and normal), or null. */
export function readDenied(path: string, places: GuardPlaces): string | null {
  if (PROC_SECRETS.test(path)) {
    return "Another process's environment and memory may hold a person token.";
  }
  const name = basename(path);
  // Planroom keeps a store, its pointer and a ledger only in a state directory and in the data root
  // (held below), so an app's own `store.db` elsewhere is the app's.
  if (
    (STORE_FILE.test(name) || name === "store.pointer") &&
    (inStateDir(path, places) || globbedFolder(path))
  ) {
    return "Planroom's store is read through the planroom CLI, never directly.";
  }
  if (inStateDir(path, places) && name === "serve.json") {
    return "serve.json is the canvas server's record; an older engine's copy may still hold its control token, which now lives under the data root at serve/<port>.json.";
  }
  const data = places.dataRoot;
  for (const sub of DATA_SECRETS) {
    if (within(path, `${data}/${sub}`)) {
      return "The Planroom data root's stores, ledger, backups, login pages and serve control tokens are off limits.";
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
  "The Planroom plugin's own files (the mod with its guard and person key code, and the resolver every press runs), its other versions and its marketplace copy change only when the person installs or updates the plugin.";

/**
 * The plugin's folders an agent never writes (security-r3-3): its root as the host names it and
 * where it lands, and, for a root in the host's plugin cache (`<config>/plugins/cache/<market>/
 * <plugin>/<version>`), every version beside it and the marketplace clone a later update installs
 * from, since the host may load either in a later session.
 */
function pluginFolders(places: GuardPlaces): string[] {
  const out = [...places.plugin];
  for (const p of places.plugin) {
    const m = /^(.*)\/plugins\/cache\/([^/]+)\/([^/]+)\/[^/]+$/.exec(p);
    if (m !== null) {
      const [, config, market, plugin] = m;
      out.push(
        `${config}/plugins/cache/${market}/${plugin}`,
        `${config}/plugins/marketplaces/${market}`,
      );
    }
  }
  return [...new Set(out)];
}

/** The host's lists of installed plugins and marketplaces, which pick the plugin code it loads. */
function hostPluginLists(places: GuardPlaces): string[] {
  return ["installed_plugins.json", "known_marketplaces.json"].map(
    (n) => `${places.claudeConfig}/plugins/${n}`,
  );
}

/** Why an agent may not write this path (absolute and normal), or null. */
export function writeDenied(path: string, places: GuardPlaces): string | null {
  if (inStateDir(path, places)) return STATE_WRITE;
  if (within(path, places.dataRoot)) return DATA_WRITE;
  if (
    pluginFolders(places).some((p) => within(path, p)) ||
    hostPluginLists(places).includes(path)
  ) {
    return PLUGIN_WRITE;
  }
  return readDenied(path, places);
}

/** Whether a path is in a state directory's `share/` folder, which `share` and `import` write. */
function inShareDir(path: string, places: GuardPlaces): boolean {
  const p = segmentsOf(path);
  return (
    stateDirEnds(p, same).some((k) => p[k + 1] === "share") ||
    namedGitDirs(places).some(({ state }) => within(path, `${state}/share`))
  );
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
  if (inStateDir(path, places) && !inShareDir(path, places)) {
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
  return mayBeInStateDir(path, places) ||
    heldFolders(places).some((h) => mayName(path, h, "within"))
    ? "An agent doesn't cd into Planroom's state, its data root, the mod's store, the user config's folder or the plugin's own folder; name the file from where you are, or use the planroom CLI."
    : null;
}

/** The folders, beside state directories, that no shell sits in: the data root, the mod's store, the user config's and the plugin's. */
function heldFolders(places: GuardPlaces): string[] {
  const config = places.userConfig.slice(0, places.userConfig.lastIndexOf("/"));
  return [
    places.dataRoot,
    modStoreDir(places),
    config,
    ...pluginFolders(places),
  ];
}

/**
 * Why the shell may run nothing from where it sits, or null. In a protected folder any program,
 * `make` or a script, writes through names the guard never sees, so only a `cd` out runs there.
 */
function shellSitsDenied(places: GuardPlaces): string | null {
  const cwd = places.cwd;
  return inStateDir(cwd, places) ||
    heldFolders(places).some((h) => within(cwd, h))
    ? `The shell sits in a folder Planroom protects, where any program may change its files. Start the line with a cd out of it, such as cd ${places.workspace}.`
    : null;
}

/**
 * The places a crafted tree mustn't land above: what it unpacks or moves there could replace the
 * data root (and the engine every press runs), the mod's store, the user config or the plugin.
 */
function landingTargets(places: GuardPlaces): string[] {
  return [
    places.dataRoot,
    modStoreDir(places),
    places.userConfig,
    ...pluginFolders(places),
  ];
}

const UNPACK =
  "This unpacks or moves files the guard can't see into a folder above Planroom's data root, the person key, the gateway key or the Planroom plugin, so it could replace them. Unpack into a folder that holds none of them.";

/** Why an archive may not unpack into this folder (absolute and normal), or null. */
function unpackDenied(path: string, places: GuardPlaces): string | null {
  return landingTargets(places).some((t) => mayName(path, t, "above"))
    ? UNPACK
    : null;
}

/** Something `mv` moves: the source's name, and the folder it goes to as written. */
export interface Arrival {
  name: string;
  dest: string;
  /** Whether the destination must be an existing folder (`dir/`, or `-t dir`), so nothing is renamed. */
  folder: boolean;
}

/**
 * Why `mv` may not move something named `name` into `dest` (absolute and normal), or null: it
 * would land on the path to a protected place, such as `mv crafted/.local ~/`. A destination
 * strictly between the home folder and that place, not written as a folder, is refused whatever
 * the name, since a `mv` to one that doesn't exist yet renames the source into it.
 */
function arrivalDenied(
  arrival: Arrival,
  dest: string,
  places: GuardPlaces,
): string | null {
  const d = segmentsOf(dest);
  for (const target of landingTargets(places)) {
    const t = segmentsOf(target);
    if (d.length >= t.length || !mayName(dest, target, "above")) continue;
    if (segmentMay(arrival.name, t[d.length] ?? "")) return UNPACK;
    if (!arrival.folder && within(dest, places.home) && dest !== places.home) {
      return UNPACK;
    }
  }
  return null;
}

/**
 * The files an agent never reads, by any tool, nor reaches through a folder above them: the mod's
 * `$.store` folder (the person key), the user-level config (the gateway key), and the data root's
 * stores, backups, login pages, serve records and ledger (security-r2-2).
 */
function secretPlaces(places: GuardPlaces): string[] {
  const data = places.dataRoot;
  return [
    modStoreDir(places),
    places.userConfig,
    ...DATA_SECRETS.map((s) => `${data}/${s}`),
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

/** `**` as a line that turns dotglob on spells it: it then matches hidden names too. */
const DOT_GLOBSTAR = "{,}**";

/** Whether a glob segment is `**`, which Bash's globstar reads as any number of folders. */
function isGlobstar(segment: string): boolean {
  return segment === "**" || segment === DOT_GLOBSTAR;
}

/**
 * Whether `path` (absolute and normal, maybe a glob) may name `target` or something inside it
 * ("within"), `target` or a folder above it ("above"), or `target` itself ("is"). A `**` stands
 * for no folder or any number of them, none hidden unless the line turns dotglob on, as in Bash:
 * `src/**` never reaches `src/.planroom`, and `dir/**` names `dir/` itself.
 */
function mayName(
  path: string,
  target: string,
  mode: "within" | "above" | "is",
): boolean {
  const p = segmentsOf(path);
  const t = segmentsOf(target);
  const seen = new Map<number, boolean>();
  const from = (i: number, j: number): boolean => {
    const key = i * (t.length + 1) + j;
    const held = seen.get(key);
    if (held !== undefined) return held;
    let out: boolean;
    if (j === t.length) {
      out = mode === "within" || p.slice(i).every(isGlobstar);
    } else if (i === p.length) {
      // `mode` is "above" or "is" here: the path stops at a folder above the target.
      out = mode === "above";
    } else {
      const segment = p[i] ?? "";
      const name = t[j] ?? "";
      out = isGlobstar(segment)
        ? from(i + 1, j) || (segmentMatches(segment, name) && from(i, j + 1))
        : segmentMay(segment, name) && from(i + 1, j + 1);
    }
    seen.set(key, out);
    return out;
  };
  return from(0, 0);
}

/**
 * Whether a path, maybe a glob, may lie in a state directory: `.planroom/`, or `planroom/` in a
 * repo's, a linked worktree's or a submodule's git directory, or in the git directory a `.git`
 * file names.
 */
function mayBeInStateDir(path: string, places: GuardPlaces): boolean {
  return (
    stateDirEnds(segmentsOf(path), segmentMay).length > 0 ||
    namedGitDirs(places).some(({ state }) => mayName(path, state, "within"))
  );
}

/** Why an agent may not write, create or change what this path names, maybe a glob, or null. */
function writeTargetDenied(path: string, places: GuardPlaces): string | null {
  if (!GLOB.test(path)) return writeDenied(path, places);
  if (mayBeInStateDir(path, places)) return STATE_WRITE;
  if (mayName(path, places.dataRoot, "within")) return DATA_WRITE;
  if (
    [...pluginFolders(places), ...hostPluginLists(places)].some((p) =>
      mayName(path, p, "within"),
    )
  ) {
    return PLUGIN_WRITE;
  }
  return globDenied(path, places);
}

/** Why an agent may not delete or move away this folder with all that's inside it, or null. */
function removeDenied(path: string, places: GuardPlaces): string | null {
  const held = [
    places.dataRoot,
    ...ownStateFolders(places),
    modStoreDir(places),
    places.userConfig,
    ...pluginFolders(places),
  ];
  // In the workspace's own `.git`, a linked worktree's or a submodule's git directory holds its state.
  const gitState = [places.workspace, places.cwd].some(
    (w) => mayName(path, `${w}/.git`, "within") && mayHoldState(path, places),
  );
  return gitState || held.some((h) => mayName(path, h, "above"))
    ? "This deletes a folder that holds Planroom's state, its data root, the person key, the gateway key or the Planroom plugin."
    : null;
}

/**
 * The state directories of the workspace and of the repo the shell is in: `.planroom/`, the
 * repo's `.git/planroom/`, and `planroom/` in the git directory a `.git` file names.
 */
function ownStateFolders(places: GuardPlaces): string[] {
  return [
    ...[places.workspace, places.cwd].flatMap((w) => [
      `${w}/.planroom`,
      `${w}/.git/planroom`,
    ]),
    ...namedGitDirs(places).map(({ state }) => state),
  ];
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
  const reaches = (secret: string, dots = hidden): boolean =>
    mayName(path, secret, "above") &&
    (dots ||
      path.includes("**") ||
      !segmentsOf(secret)
        .slice(segmentsOf(path).length)
        .some((s) => s.startsWith(".")));
  // The state directory in a git directory a `.git` file names, which may lie under no hidden
  // folder, as a bare repo's worktree or a separate git directory does: held as a secret place
  // where the path to it crosses none. A read whose path to it crosses a hidden folder, or one
  // rooted at the workspace, stays as the workspace's own `.planroom/` leaves it.
  const named =
    path !== places.workspace &&
    namedGitDirs(places).some(({ state }) => reaches(state, false));
  // A state directory, or a folder above one in a git directory: `.git`, `.git/worktrees/wt`.
  return mayHoldState(path, places) ||
    named ||
    secretPlaces(places).some((secret) => reaches(secret))
    ? "This reads, copies or links a whole folder that holds Planroom's private files: the person key, the gateway key, the stores, login pages or serve's token. Name a folder inside it that holds none of them."
    : null;
}

/**
 * A line with each backslash-newline the shell drops taken out: one whose backslash isn't itself
 * escaped, so `x\\` then a newline still ends the command. It reads quotes as nothing: Bash keeps a
 * backslash-newline inside single quotes as text, but a quoted `sh -c` script's own shell drops it,
 * so the text-only checks, which can't tell the two apart, read it dropped.
 */
function joinLines(text: string): string {
  return text.replace(/(?<!\\)((?:\\\\)*)\\\n/g, "$1");
}

/**
 * A shell command with its quotes and backslash escapes taken out, so a name split by quotes
 * (`PLANROOM_PERSON"_TOKEN"`, `plan'room'`) or by a backslash-newline reads as the shell joins it.
 */
export function unquote(command: string): string {
  return joinLines(command).replace(/["'\\]/g, "");
}

/**
 * `unquote(command)`, with whether the shell reads each of its characters inside quotes or after a
 * backslash, so a quoted space can be told from one that ends a word. A backslash-newline is
 * dropped as the shell drops it: outside single quotes, and only when the backslash isn't escaped.
 */
function unquoteMarked(command: string): { text: string; quoted: boolean[] } {
  const source = command;
  let text = "";
  const quoted: boolean[] = [];
  let state: "'" | '"' | null = null;
  let escaped = false;
  // By UTF-16 unit, as shellCommands reads `text`.
  for (let i = 0; i < source.length; i++) {
    const ch = source.charAt(i);
    if (
      ch === "\\" &&
      !escaped &&
      state !== "'" &&
      source.charAt(i + 1) === "\n"
    ) {
      i++;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "\\") {
      // unquote drops every one of these; only the shell's own reading moves the state.
      if (escaped) escaped = false;
      else if (ch === "\\" && state !== "'") escaped = true;
      else if (state === null && ch !== "\\") state = ch;
      else if (state === ch) state = null;
      continue;
    }
    text += ch;
    quoted.push(state !== null || escaped);
    escaped = false;
  }
  return { text, quoted };
}

/** The words of a shell command, quotes and operators taken away, so `sh -c '...'` shows through. */
export function shellWords(command: string): string[] {
  return joinLines(command)
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
 * binary every later session runs (DR-64, DR-69). Every rule and message that names them reads
 * this list.
 */
const GUARDED_VARIABLES = [
  "PLANROOM_PERSON_TOKEN",
  "PLANROOM_RAW",
  "PLANROOM_ENGINE",
  "PLANROOM_RELEASE_BASE_URL",
] as const;

/**
 * Variables that place the data root, and with it the engine cache the resolver runs a press
 * from (plugin/scripts/planroom): no shell start-up or settings file an agent writes sets them,
 * though a line may use them.
 */
const PLACE_VARIABLES = ["PLANROOM_DATA", "XDG_DATA_HOME", "HOME"] as const;

/** Names as prose: `A, B or C`. */
function spelled(names: readonly string[]): string {
  return names.length < 2
    ? names.join("")
    : `${names.slice(0, -1).join(", ")} or ${names.at(-1) ?? ""}`;
}

const GUARDED_ALTERNATIVES = GUARDED_VARIABLES.join("|");

/** A Bash line that sets one of `GUARDED_VARIABLES`, by assignment or by a builtin that sets. */
const GUARDED_SET = new RegExp(
  [
    `\\b(${GUARDED_ALTERNATIVES})\\b\\s*\\+?=`,
    `\\b(export|declare|typeset|env|set|readonly|local|read)\\b[^;&|\\n]*\\b(${GUARDED_ALTERNATIVES})\\b`,
    `\\bprintf\\b[^;&|\\n]*-v\\s*(${GUARDED_ALTERNATIVES})\\b`,
  ].join("|"),
);

const GUARDED_SET_REASON = `An agent never sets ${spelled(GUARDED_VARIABLES)}.`;

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

/**
 * The command lines socat's `EXEC:`, `SYSTEM:` and `SHELL:` addresses run, in a line with its
 * quotes taken out: each value up to its first option (`,`) or the dual-address `!!`. socat reads
 * an address's keyword in any case, and each value is shorter than the line it came from.
 */
function socatCommands(plain: string): string[] {
  if (!shellWords(plain).some((w) => basename(w) === "socat")) return [];
  return [
    ...plain.matchAll(/(?:^|[\s"'])(?:exec|system|shell):([^,!]*)/gi),
  ].map((m) => m[1] ?? "");
}

/** The guard's verdict on a Bash command's text alone, before its paths are resolved. */
export function bashDenied(command: string): string | null {
  const plain = unquote(command);
  if (GUARDED_SET.test(plain)) return GUARDED_SET_REASON;
  const words = shellWords(plain);
  for (const line of planroomLines(words)) {
    const reason = personOnlyLine(line);
    if (reason !== null) return reason;
  }
  // socat runs an `EXEC:`, `SYSTEM:` or `SHELL:` address's value as a command line, under a pty
  // with `,pty`: `socat - EXEC:'planroom open',pty` is a pty-wrapped person-only command.
  for (const line of socatCommands(plain)) {
    const reason = bashDenied(line);
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
    // security-r2-4: a planroom whose command words come from stdin (`xargs`, `parallel`, `find
    // -exec`) or from an expansion (`planroom "$@"`) may be a person-only command the line hides.
    const start = commandStart(simple);
    for (const [i, word] of simple.entries()) {
      if (basename(word) !== "planroom") continue;
      const fed =
        simple.slice(0, i).some((w) => FEEDERS.has(basename(w))) ||
        FIND_ACTIONS.has(simple[i - 1] ?? "");
      if (!fed && i !== start) continue;
      let first: string | undefined;
      for (let j = i + 1; j < simple.length; j++) {
        const w = simple[j] ?? "";
        if (VALUE_FLAGS.has(w)) j++;
        else if (!w.startsWith("-")) {
          first = w;
          break;
        }
      }
      if (fed || (first !== undefined && isExpansion(first))) {
        return "Spell out planroom's command on the line: one fed from stdin or a variable may be the person's alone.";
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
    ...(places.zdotdir === undefined
      ? []
      : [["ZDOTDIR", places.zdotdir] as [string, string]]),
    // The planroom skill names the resolver through it; a shell that has it set has this plugin's.
    ...places.plugin
      .slice(0, 1)
      .map((p): [string, string] => ["CLAUDE_PLUGIN_ROOT", p]),
  ]);
}

/**
 * What unknown expansions joined in one name stand for, to each other (`$A$B`) or to other text
 * (`.$A`): any name, a hidden one too, as the dotglob form spells it, and never `**`, which `**`'s
 * own rules would keep out of hidden folders. `fnmatch` reads its `{,}` as nothing.
 */
const ANY_JOINED = "{,}*{,}";

/**
 * A glob (Bash's pattern in `${X#pat}` and `${X/pat/rep}`) as an anchored test of a whole string.
 */
function globTest(pattern: string): (text: string) => boolean {
  return (text) => fnmatch(pattern, text, false);
}

/**
 * A known value with a Bash pattern operator applied: `#`/`##` and `%`/`%%` remove the shortest or
 * longest matching prefix or suffix, and `/`, `//`, `/#` and `/%` replace the longest match, all
 * of them, the prefix or the suffix. Undefined for any other operator, or a value too long to try.
 */
function applyOperator(value: string, op: string): string | undefined {
  if (value.length > 256) return undefined;
  const removal = /^(##?|%%?)(.*)$/s.exec(op);
  if (removal !== null) {
    const [, kind = "", pattern = ""] = removal;
    const test = globTest(pattern);
    const n = value.length;
    const cuts = [...Array(n + 1).keys()];
    const longest = kind.length === 2;
    if (kind.startsWith("#")) {
      const at = (longest ? cuts.reverse() : cuts).find((i) =>
        test(value.slice(0, i)),
      );
      return at === undefined ? value : value.slice(at);
    }
    const at = (longest ? cuts : cuts.reverse()).find((i) =>
      test(value.slice(i)),
    );
    return at === undefined ? value : value.slice(0, at);
  }
  // `/pat/rep`: an escaped `/` in the pattern is gone with its backslash by now, so a remainder
  // with more than one `/` can't be split as Bash splits it.
  const swap = /^\/(\/|#|%)?([^/]*)(?:\/([^/]*))?$/s.exec(op);
  if (swap === null) return undefined;
  const [, mode = "", pattern = "", rep = ""] = swap;
  const test = globTest(pattern);
  /** Where the longest match from `start` ends, if one does and isn't empty. */
  const longestAt = (start: number): number | undefined => {
    for (let end = value.length; end > start; end--) {
      if (test(value.slice(start, end))) return end;
    }
    return undefined;
  };
  if (mode === "#") {
    const end = longestAt(0);
    return end === undefined ? value : rep + value.slice(end);
  }
  if (mode === "%") {
    for (let start = 0; start < value.length; start++) {
      if (test(value.slice(start))) return value.slice(0, start) + rep;
    }
    return value;
  }
  let out = "";
  let i = 0;
  while (i < value.length) {
    const end = longestAt(i);
    if (end === undefined) {
      out += value.charAt(i);
      i++;
      continue;
    }
    out += rep;
    i = end;
    if (mode !== "/") return out + value.slice(i);
  }
  return out;
}

/**
 * `word` with each variable it names put in: a known one's value, one the line assigned before
 * its value, and any other expansion, since it could hold anything, as `*` when it is a whole name
 * on its own (a variable set outside the line, which architecture.md's "Actors" says the guard
 * doesn't read), and as `ANY_JOINED` when it's joined to another in one name. A known value
 * stands in only plain (`$N`, `${N}`) or behind a default (`${N:-word}`), which a set variable
 * ignores; a pattern operator on it (`${PWD/*\/.planroom}`) is applied, and any other operator
 * (`${PWD:0:0}`) gives `ANY_JOINED`, even as a whole name.
 */
function expand(word: string, vars: ReadonlyMap<string, string>): string {
  // Characters no word holds, noncharacters a shell line never spells: an unknown expansion, and
  // one that may be any name even on its own.
  const unknown = "\uffff";
  const anyName = "\ufffe";
  const put = word
    .replace(
      /\$\{([A-Za-z_][A-Za-z0-9_]*)([^}]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g,
      (
        _,
        braced: string | undefined,
        op: string | undefined,
        bare: string | undefined,
      ) => {
        const value = vars.get(braced ?? bare ?? "");
        if (op === undefined || op === "" || /^:?[-=?]/.test(op)) {
          return value ?? unknown;
        }
        const applied =
          value === undefined || GLOB.test(value)
            ? undefined
            : applyOperator(value, op);
        return applied ?? anyName;
      },
    )
    .replace(/\$[^/]*|`[^/]*/g, unknown);
  if (!put.includes(unknown) && !put.includes(anyName)) return put;
  return put
    .split("/")
    .map((s) =>
      s === unknown ? "*" : s.replace(/[\uffff\ufffe]+/g, ANY_JOINED),
    )
    .join("/");
}

/** One simple command of a Bash line: its words, and the files its redirections name. */
interface SimpleCommand {
  words: string[];
  /**
   * For each word, whether it continues the shell word before it, split from it only by quoted
   * white space: `'s/a b/c/'` is the words `s/a` and `b/c/`, the second joined.
   */
  joined: boolean[];
  /** Files a `>`, `>>`, `>|`, `&>`, `>&FILE` or `<>` writes. */
  writes: string[];
  /** Files a `<` reads. */
  reads: string[];
  /** The operator that ended it: `;`, `|`, `&`, `(`, `)`, a backtick, a newline, or none. */
  sep: string;
}

/**
 * The simple commands of a Bash line, quotes taken out: operators, parentheses and backticks split
 * it, and each redirection's file is kept apart from the command's words. A here-document's
 * delimiter, a here-string and a duplicated descriptor (`2>&1`) name no file.
 */
function shellCommands(command: string): SimpleCommand[] {
  const { text, quoted } = unquoteMarked(command);
  const out: SimpleCommand[] = [];
  const fresh = (): SimpleCommand => ({
    words: [],
    joined: [],
    writes: [],
    reads: [],
    sep: "",
  });
  let cmd = fresh();
  let cur = "";
  let into: "word" | "write" | "read" | "skip" | "dup" = "word";
  // Whether the shell word being read has given a word yet, and whether the next continues it.
  let started = false;
  let glue = false;
  const flush = (): void => {
    if (cur === "") return;
    if (into === "word") {
      cmd.words.push(cur);
      cmd.joined.push(glue);
      glue = false;
      started = true;
    } else if (into === "write") cmd.writes.push(cur);
    else if (into === "read") cmd.reads.push(cur);
    else if (into === "dup" && !/^(\d+-?|-)$/.test(cur)) cmd.writes.push(cur);
    cur = "";
    into = "word";
  };
  const end = (sep: string): void => {
    flush();
    into = "word";
    started = false;
    glue = false;
    if (cmd.words.length + cmd.writes.length + cmd.reads.length > 0) {
      out.push({ ...cmd, sep });
    }
    cmd = fresh();
  };
  // A redirection's operator: a descriptor number just before it belongs to it, not to the words.
  const operator = (): void => {
    if (/^\d+$/.test(cur)) cur = "";
    else flush();
    started = false;
    glue = false;
  };
  for (let i = 0; i < text.length; i++) {
    const ch = text.charAt(i);
    const next = text.charAt(i + 1);
    if (";|()`\n".includes(ch)) {
      end(ch);
    } else if (/\s/.test(ch)) {
      flush();
      if (quoted[i] === true) glue = started;
      else {
        started = false;
        glue = false;
      }
    } else if (ch === "&" && next === ">") {
      operator();
      i += text.charAt(i + 2) === ">" ? 2 : 1;
      into = "write";
    } else if (ch === "&") {
      end(ch);
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
  end("");
  return out;
}

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

/**
 * A command that runs the command after it: its options that take a value, by letter and by long
 * name, its long options that take none (so an abbreviation is read as getopt reads it), the one
 * that runs that command in another folder, as a `cd` would, and env's `-S`, whose value is the
 * command line itself.
 */
interface Wrapper {
  short: string;
  long: readonly string[];
  plain?: readonly string[];
  chdir?: { short: string; long: string };
  split?: { short: string; long: string };
}

const WRAPPERS = new Map<string, Wrapper>([
  [
    "sudo",
    {
      short: "ugChpUDrtTR",
      long: [
        "--user",
        "--group",
        "--close-from",
        "--host",
        "--prompt",
        "--other-user",
        "--chdir",
        "--role",
        "--type",
        "--command-timeout",
        "--chroot",
      ],
      plain: [
        "--askpass",
        "--background",
        "--bell",
        "--edit",
        "--help",
        "--login",
        "--list",
        "--non-interactive",
        "--preserve-groups",
        "--preserve-env",
        "--remove-timestamp",
        "--reset-timestamp",
        "--shell",
        "--stdin",
        "--validate",
        "--version",
      ],
      chdir: { short: "D", long: "--chdir" },
    },
  ],
  ["doas", { short: "uC", long: [] }],
  [
    "env",
    {
      short: "uCS",
      long: ["--unset", "--chdir", "--split-string"],
      plain: [
        "--ignore-environment",
        "--null",
        "--debug",
        "--help",
        "--version",
        "--block-signal",
        "--default-signal",
        "--ignore-signal",
        "--list-signal-handling",
      ],
      chdir: { short: "C", long: "--chdir" },
      split: { short: "S", long: "--split-string" },
    },
  ],
  [
    "xargs",
    {
      short: "InPLsEda",
      long: [
        "--max-args",
        "--max-procs",
        "--max-lines",
        "--max-chars",
        "--eof",
        "--delimiter",
        "--arg-file",
      ],
    },
  ],
  ["timeout", { short: "sk", long: ["--signal", "--kill-after"] }],
  ["nice", { short: "n", long: ["--adjustment"] }],
  ["command", { short: "", long: [] }],
  ["builtin", { short: "", long: [] }],
  ["exec", { short: "a", long: [] }],
  ["nohup", { short: "", long: [] }],
  ["time", { short: "", long: [] }],
  ["stdbuf", { short: "ioe", long: ["--input", "--output", "--error"] }],
]);

/**
 * A long option's full name, as getopt takes it: the name itself, or the one known name it
 * abbreviates past two letters (`--chd` for `--chdir`). An ambiguous or unknown one is itself.
 */
function longOption(name: string, wrapper: Wrapper): string {
  const known = [...wrapper.long, ...(wrapper.plain ?? [])];
  if (known.includes(name) || name.length <= 4) return name;
  const matches = known.filter((k) => k.startsWith(name));
  return matches.length === 1 ? (matches[0] ?? name) : name;
}

/**
 * A wrapper's own words from `from` on: where the command it runs starts, each folder an option
 * (`env -C DIR`, `sudo --chdir=DIR`) has it run there, and whether `env -S` came with its line
 * attached (`-S'rm x'`), which the shell's words no longer show as the line env splits.
 */
function wrapperOptions(
  words: readonly string[],
  from: number,
  wrapper: Wrapper,
): { at: number; dirs: string[]; attachedSplit: boolean } {
  const dirs: string[] = [];
  let attachedSplit = false;
  let i = from;
  for (; i < words.length; i++) {
    const a = words[i] ?? "";
    if (a === "--") {
      i++;
      break;
    }
    // `env -` is `env -i`: it clears the environment and starts no command.
    if (a === "-") continue;
    if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      const name = longOption(eq < 0 ? a : a.slice(0, eq), wrapper);
      if (name === wrapper.split?.long) {
        // `--split-string LINE`: the command starts at its value, which the shell split already.
        if (eq >= 0) attachedSplit = true;
        else i++;
        break;
      }
      const valued = wrapper.long.includes(name);
      const value =
        eq >= 0 ? a.slice(eq + 1) : valued ? words[i + 1] : undefined;
      if (valued && eq < 0) i++;
      if (name === wrapper.chdir?.long && value !== undefined) dirs.push(value);
    } else if (a.length > 1 && a.startsWith("-")) {
      // A cluster (`-iC DIR`, `-iCDIR`): a letter that takes a value takes the rest, or the next word.
      let split = false;
      for (let c = 1; c < a.length; c++) {
        const letter = a.charAt(c);
        if (!wrapper.short.includes(letter)) continue;
        const last = c === a.length - 1;
        if (letter === wrapper.split?.short) {
          split = true;
          if (!last) attachedSplit = true;
          break;
        }
        const value = last ? words[i + 1] : a.slice(c + 1);
        if (last) i++;
        if (letter === wrapper.chdir?.short && value !== undefined) {
          dirs.push(value);
        }
        break;
      }
      if (split) {
        i++;
        break;
      }
    } else if (!ASSIGNMENT.test(a)) break;
  }
  return { at: i, dirs, attachedSplit };
}

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

/**
 * Where the command itself starts in a simple command's words, after assignments and wrappers,
 * each folder a wrapper runs it in, and whether `env -S` hid its line in one word.
 */
function commandParts(words: readonly string[]): {
  start: number;
  dirs: string[];
  attachedSplit: boolean;
} {
  const dirs: string[] = [];
  let attachedSplit = false;
  let i = 0;
  while (i < words.length) {
    const word = words[i] ?? "";
    const name = basename(word);
    const wrapper = WRAPPERS.get(name);
    if (ASSIGNMENT.test(word) || KEYWORDS.has(word)) {
      i++;
    } else if (wrapper !== undefined) {
      const own = wrapperOptions(words, i + 1, wrapper);
      i = own.at;
      dirs.push(...own.dirs);
      attachedSplit ||= own.attachedSplit;
      // timeout's duration
      if (name === "timeout" && /^\d/.test(words[i] ?? "")) i++;
    } else if (
      SHELLS.has(name) &&
      /^-[a-z]*c[a-z]*$/.test(words[i + 1] ?? "")
    ) {
      // `sh -c '...'`: its quotes are gone, so the words after `-c` are the command.
      i += 2;
    } else {
      break;
    }
  }
  return { start: i, dirs, attachedSplit };
}

/** Where the command itself starts in a simple command's words: after assignments and wrappers. */
function commandStart(words: readonly string[]): number {
  return commandParts(words).start;
}

/** A tree a command reads, copies or links as a whole, and whether it reaches hidden names in it. */
export interface Tree {
  word: string;
  /** False for `rg`, `ag` and `fd` without a flag that includes hidden files: they skip dot names. */
  hidden: boolean;
}

/** A `find` that deletes or moves away what it selects: its roots as written, and its selection. */
export interface FindDelete {
  roots: string[];
  /**
   * Whether it selects this path, as find spells it under one of its roots, `depth` below it, a
   * folder or not, empty or not.
   */
  selects: Selection;
}

type Selection = (
  path: string,
  dir: boolean,
  depth: number,
  empty: boolean,
) => boolean;

/** What a Bash line's commands do to the paths they name, beyond reading them (security-r2-1, -2). */
export interface BashEffects {
  /** Files and folders a command may write, create, change or delete, redirections included. */
  writes: string[];
  /** Folders a command deletes or moves away whole, with all that's inside. */
  removes: string[];
  /** Folders a command reads, archives, copies or links as a whole tree. */
  trees: Tree[];
  /** Folders an archive unpacks into, with contents the line doesn't show. */
  unpacks: string[];
  /** What `mv` moves, and where. */
  arrivals: Arrival[];
  /** `find`s that delete or move away what they select (security-r3-4). */
  finds: FindDelete[];
  /** Why the line is denied outright, whatever its paths, or null. */
  refusal: string | null;
  /** Why the person decides the line, whatever its paths, when nothing denies it, or null. */
  question: string | null;
}

function noEffects(): BashEffects {
  return {
    writes: [],
    removes: [],
    trees: [],
    unpacks: [],
    arrivals: [],
    finds: [],
    refusal: null,
    question: null,
  };
}

const SPLIT_ATTACHED =
  "env -S with its command line in the same word runs a line the Planroom guard can't read as env splits it, so you decide whether it runs.";

const REMOVERS = new Set([
  "rm",
  "rmdir",
  "unlink",
  "shred",
  "trash",
  "trash-put",
]);
/** Commands that delete or move away what they're given. */
const DISPOSERS = new Set([...REMOVERS, "mv"]);
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
  "bsdtar",
]);
/** Commands that write their last operand, or the folder `-t` names. */
const COPIERS = new Set(["cp", "install", "ln", "rsync", "scp", "ditto"]);
const IN_PLACE = new Set(["sed", "perl", "ruby"]);
const SEARCHERS = new Set([
  "grep",
  "egrep",
  "fgrep",
  "zgrep",
  "rgrep",
  "ack",
  "ugrep",
  "ug",
]);
/** Searchers that skip hidden names unless a flag says otherwise. */
const SKIPPERS = new Set(["rg", "ag", "fd", "fdfind"]);
const FIND_ACTIONS = new Set(["-exec", "-execdir", "-ok", "-okdir"]);
/** `find`'s tests whose value is a pattern, not a path it opens. */
const FIND_TESTS = new Set([
  "-name",
  "-iname",
  "-path",
  "-ipath",
  "-wholename",
  "-iwholename",
  "-regex",
  "-iregex",
]);
/** Commands that run another with words they read from stdin. */
const FEEDERS = new Set(["xargs", "parallel"]);
/** Compressors whose `-r` reads, and rewrites, a whole tree. */
const COMPRESSORS = new Set([
  "gzip",
  "gunzip",
  "pigz",
  "unpigz",
  "zstd",
  "unzstd",
  "lz4",
  "bzip2",
  "xz",
]);
const SEVEN_ZIP = new Set(["7z", "7za", "7zz", "7zr"]);

/**
 * Whether `git clean`'s words ask for a dry run that deletes nothing: its options up to `--`, the
 * last of `-n`, `--dry-run` and `--no-dry-run` winning, each long one also as git takes it
 * abbreviated (`--dry`, `--no-d`).
 */
function dryRun(rest: readonly string[]): boolean {
  /** Whether `word` is `--NAME`, or git's abbreviation of it. */
  const abbreviates = (word: string, prefix: string, name: string): boolean =>
    word.length > prefix.length &&
    word.startsWith(prefix) &&
    name.startsWith(word.slice(prefix.length));
  let dry = false;
  for (let k = 0; k < rest.length; k++) {
    const a = rest[k] ?? "";
    if (a === "--") break;
    if (a === "-e") k++;
    else if (abbreviates(a, "--no-", "dry-run")) dry = false;
    else if (abbreviates(a, "--", "dry-run")) dry = true;
    // `--exclude PATTERN`, abbreviated or not; `--exclude=PATTERN` is one word.
    else if (abbreviates(a, "--", "exclude")) k++;
    else if (/^-[^-]/.test(a)) {
      // `-e` takes the rest of its cluster as a pattern, else the next word.
      for (let c = 1; c < a.length; c++) {
        if (a.charAt(c) === "e") {
          if (c === a.length - 1) k++;
          break;
        }
        if (a.charAt(c) === "n") dry = true;
      }
    }
  }
  return dry;
}

/** `git`'s own `-C` folder, its subcommand, and the words after it. */
function gitParts(args: readonly string[]): {
  dir: string | undefined;
  sub: string;
  rest: string[];
} {
  let dir: string | undefined;
  for (let i = 0; i < args.length; i++) {
    const a = args[i] ?? "";
    if (a === "-C") {
      const v = args[i + 1];
      if (v !== undefined) {
        dir = dir === undefined || /^[/~]/.test(v) ? v : `${dir}/${v}`;
      }
      i++;
    } else if (["-c", "--git-dir", "--work-tree", "--namespace"].includes(a)) {
      i++;
    } else if (!a.startsWith("-")) {
      return { dir, sub: a, rest: args.slice(i + 1) };
    }
  }
  return { dir, sub: "", rest: [] };
}

/** A shell glob (`find`'s `-name` and `-path`) as a test: `*` matches a leading dot and, in a path, a `/`. */
function fnmatch(glob: string, text: string, ci: boolean): boolean {
  const escape = (c: string): string =>
    c.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  const pattern = glob.split("{,}").join("");
  let re = "";
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern.charAt(i);
    if (ch === "*") re += ".*";
    else if (ch === "?") re += ".";
    else if (ch === "[") {
      const end = pattern.indexOf("]", i + 2);
      if (end < 0) {
        re += "\\[";
        continue;
      }
      const set = pattern.slice(i + 1, end).replace(/^!/, "^");
      re += `[${set.replace(/\\/g, "\\\\")}]`;
      i = end;
    } else if (ch === "\\" && i + 1 < pattern.length) {
      i++;
      re += escape(pattern.charAt(i));
    } else re += escape(ch);
  }
  try {
    return new RegExp(`^${re}$`, ci ? "i" : "").test(text);
  } catch {
    return true;
  }
}

interface FindTest {
  test: (path: string, dir: boolean, empty: boolean) => boolean;
  negated: boolean;
}

/**
 * What a `find` expression selects, read the safe way: its `-o` groups, each the tests it ANDs,
 * `!`/`-not` turning one over, `-prune` keeping what its group matches out with all below it,
 * and `-maxdepth`/`-mindepth`. `-empty` selects an empty file, and `-size 0` an empty file or a
 * folder, which may report a size of 0; a test the guard doesn't read restricts nothing.
 */
function findSelection(expr: readonly string[]): Selection {
  const groups: { tests: FindTest[]; prune: boolean; acts: boolean }[] = [
    { tests: [], prune: false, acts: false },
  ];
  let negate = false;
  let max = Infinity;
  let min = 0;
  for (let i = 0; i < expr.length; i++) {
    const a = expr[i] ?? "";
    const value = expr[i + 1] ?? "";
    const group = groups.at(-1) ?? { tests: [], prune: false, acts: false };
    const add = (test: FindTest["test"]): void => {
      group.tests.push({ test, negated: negate });
      negate = false;
      i++;
    };
    if (a === "!" || a === "-not") negate = !negate;
    else if (a === "-o" || a === "-or") {
      groups.push({ tests: [], prune: false, acts: false });
      negate = false;
    } else if (a === "-name" || a === "-iname") {
      add((p) => fnmatch(value, basename(p), a === "-iname"));
    } else if (["-path", "-ipath", "-wholename", "-iwholename"].includes(a)) {
      add((p) => fnmatch(value, p, a.startsWith("-i")));
    } else if (a === "-regex" || a === "-iregex") {
      add((p) => {
        // An expansion in it may be any pattern.
        if (value.includes(ANY_JOINED) || value === "*") return true;
        try {
          return new RegExp(`^(?:${value})$`, a === "-iregex" ? "i" : "").test(
            p,
          );
        } catch {
          return true;
        }
      });
    } else if (a === "-type") {
      add((_, dir) => (value === "f" ? !dir : value === "d" ? dir : true));
    } else if (a === "-empty") {
      // A protected folder holds something; a protected file may be empty, such as the data
      // root's lock or a checkpointed WAL.
      group.tests.push({
        test: (_, dir, empty) => !dir && empty,
        negated: negate,
      });
      negate = false;
    } else if (a === "-size") {
      // `-size 0`, `-size -1k`: find rounds up, so only an empty file is that small, or a folder
      // whose file system reports 0.
      if (/^(-?0|-1)[bcwkMG]?$/.test(value)) {
        add((_, dir, empty) => dir || empty);
      } else i++;
    } else if (a === "-maxdepth" || a === "-mindepth") {
      const n = Number(value);
      if (Number.isFinite(n)) {
        if (a === "-maxdepth") max = n;
        else min = n;
      }
      i++;
    } else if (a === "-prune") group.prune = true;
    else if (a === "-delete") group.acts = true;
    else if (FIND_ACTIONS.has(a)) {
      group.acts = true;
      break;
    }
  }
  const holds = (
    tests: FindTest[],
    path: string,
    dir: boolean,
    empty: boolean,
  ): boolean => tests.every((t) => t.test(path, dir, empty) !== t.negated);
  const pruned = groups.filter((g) => g.prune);
  // An action binds to its own `-o` branch (`-empty -o -name '*.orig' -delete` deletes only
  // `.orig` files), unless parentheses group the branches, which this reading doesn't follow.
  const grouped = expr.some((a) => a === "(" || a === ")" || a === LITERAL);
  const acting = groups.filter((g) => !g.prune && g.acts);
  const selecting =
    !grouped && acting.length > 0 ? acting : groups.filter((g) => !g.prune);
  return (path, dir, depth, empty) => {
    if (depth > max || depth < min) return false;
    // A pruned folder keeps itself and all below it out; a folder above a path holds it.
    const parts = path.split("/");
    for (let k = parts.length; k > 0; k--) {
      const above = parts.slice(0, k).join("/");
      const own = k === parts.length;
      if (
        pruned.some((g) =>
          holds(g.tests, above, own ? dir : true, own ? empty : false),
        )
      ) {
        return false;
      }
    }
    return selecting.some((g) => holds(g.tests, path, dir, empty));
  };
}

/** Whether a `find` expression runs a command that deletes or moves what it selects. */
function findDisposes(expr: readonly string[]): boolean {
  return expr.some((a, i) => {
    if (!FIND_ACTIONS.has(a)) return false;
    let program = basename(expr[i + 1] ?? "");
    if (SHELLS.has(program) && /^-[a-z]*c[a-z]*$/.test(expr[i + 2] ?? "")) {
      program = basename(expr[i + 3] ?? "");
    }
    return DISPOSERS.has(program);
  });
}

/** The shell words of a command's words, each as the indices of the words it's made of. */
function shellWordsOf(
  words: readonly string[],
  joined: readonly boolean[],
): { text: string; parts: number[] }[] {
  const out: { text: string; parts: number[] }[] = [];
  words.forEach((w, k) => {
    const last = out.at(-1);
    if (joined[k] === true && last !== undefined) {
      last.text += ` ${w}`;
      last.parts.push(k);
    } else out.push({ text: w, parts: [k] });
  });
  return out;
}

/** sed's long options that take no value, or one only after `=`. */
const SED_LONG = new Set([
  "--in-place",
  "--regexp-extended",
  "--quiet",
  "--silent",
  "--separate",
  "--null-data",
  "--zero-terminated",
  "--unbuffered",
  "--posix",
  "--debug",
  "--sandbox",
  "--follow-symlinks",
]);

/**
 * An in-place editor's short options, each its own: those that take nothing, those whose value is
 * its script, those that take a value (the rest of the cluster, else the next word), those whose
 * value can only be the rest of the cluster, and those that run it in another folder.
 */
interface Editor {
  plain: string;
  scripts: string;
  valued: string;
  attached: string;
  chdir: string;
}

const EDITORS = new Map<string, Editor>([
  [
    "sed",
    { plain: "nErsuz", scripts: "ef", valued: "l", attached: "", chdir: "" },
  ],
  [
    "perl",
    {
      plain: "pnalswWXtTcuUvy0123456789",
      scripts: "eE",
      valued: "",
      attached: "",
      chdir: "",
    },
  ],
  [
    // `-E` is an encoding, `-C DIR` and `-X DIR` run it in DIR, and `-x[DIR]` may too.
    "ruby",
    {
      plain: "acdlnpsSvwyUh",
      scripts: "e",
      valued: "CEFIrX",
      attached: "0WxTK",
      chdir: "CXx",
    },
  ],
]);

/**
 * Which of an in-place editor's arguments name no file it writes, and the folder it runs in when
 * an option moves it there: its options, with their values, and its script, which for sed is each
 * `-e`/`--expression` and `-f`/`--file` value, else its first operand, and for perl and ruby each
 * `-e` value. An option the guard doesn't know leaves every word a file it writes.
 */
function inPlaceScripts(
  name: string,
  args: readonly string[],
  joined: readonly boolean[],
): { notFiles: Set<number>; dir: string | undefined } {
  const words = shellWordsOf(args, joined);
  const notFiles = new Set<number>();
  const sed = name === "sed";
  const editor = EDITORS.get(name);
  const unknown = { notFiles: new Set<number>(), dir: undefined };
  if (editor === undefined) return unknown;
  let dir: string | undefined;
  let given = false;
  let ended = false;
  const operands: number[] = [];
  for (let g = 0; g < words.length; g++) {
    const word = words[g]?.text ?? "";
    if (ended || word === "-" || !word.startsWith("-")) {
      operands.push(g);
      continue;
    }
    // An option, with any value it holds (`--expression='s/ .*//'`), names no file it writes.
    notFiles.add(g);
    /** The next shell word is this option's value. */
    const value = (): void => {
      g++;
      notFiles.add(g);
    };
    if (word === "--") {
      ended = true;
    } else if (word.startsWith("--")) {
      if (!sed) return unknown;
      const eq = word.indexOf("=");
      const option = eq < 0 ? word : word.slice(0, eq);
      if (option === "--expression" || option === "--file") {
        given = true;
        if (eq < 0) value();
      } else if (option === "--line-length") {
        if (eq < 0) value();
      } else if (!SED_LONG.has(option)) return unknown;
    } else {
      for (let c = 1; c < word.length; c++) {
        const letter = word.charAt(c);
        const rest = word.slice(c + 1);
        // `-i.bak`: the rest is the backup's suffix.
        if (letter === "i") break;
        if (editor.scripts.includes(letter) || editor.valued.includes(letter)) {
          if (editor.scripts.includes(letter)) given = true;
          // The rest of the cluster is its value, else the next word is.
          const held = rest === "" ? words[g + 1]?.text : rest;
          if (rest === "") value();
          if (editor.chdir.includes(letter)) dir = held;
          break;
        }
        if (editor.attached.includes(letter)) {
          if (editor.chdir.includes(letter) && rest !== "") dir = rest;
          break;
        }
        if (!editor.plain.includes(letter)) return unknown;
      }
    }
  }
  if (sed && !given && operands[0] !== undefined) notFiles.add(operands[0]);
  return {
    notFiles: new Set([...notFiles].flatMap((g) => words[g]?.parts ?? [])),
    dir,
  };
}

/** What one simple command does to the paths it names, its words expanded. */
function commandEffects(
  all: readonly string[],
  piped: boolean,
  joined: readonly boolean[] = [],
): BashEffects {
  const effects = noEffects();
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
  /** Where an archive unpacks: the folder an option names, else the shell's directory. */
  const unpack = (into: readonly string[]): void => {
    effects.unpacks.push(...(into.length > 0 ? into : ["."]));
  };
  /**
   * An archive whose own names choose where they land, an absolute one or one a rewrite makes, may
   * unpack anywhere: as into `/`.
   */
  const unpackAnywhere = (): void => {
    effects.unpacks.push("/");
  };
  /** A search's roots: its operands after the pattern, or `.` when it names none. */
  const roots = (): string[] => {
    const given =
      values("ef", ["--regexp", "--file"]).length > 0 || long("--files");
    const named = given ? ops : ops.slice(1);
    return named.length > 0 ? named : ["."];
  };
  const target = values("t", ["--target-directory"]);
  // `gio trash`, `gio remove` and `gio move` delete or move what they name, as `rm` and `mv` do.
  const gio = name === "gio" ? (ops[0] ?? "") : "";
  if (REMOVERS.has(name) || gio === "trash" || gio === "remove") {
    const named = gio === "" ? ops : ops.slice(1);
    effects.writes.push(...named);
    effects.removes.push(...named);
  } else if (gio === "move" || gio === "rename") {
    effects.writes.push(...ops.slice(1));
    effects.removes.push(...ops.slice(1, gio === "move" ? -1 : 2));
  } else if (name === "mv") {
    effects.writes.push(...ops);
    const sources =
      target.length > 0
        ? ops.filter((o) => !target.includes(o))
        : ops.slice(0, -1);
    effects.removes.push(...sources);
    const dest = target[0] ?? (ops.length >= 2 ? ops.at(-1) : undefined);
    if (dest !== undefined) {
      effects.arrivals.push(
        ...sources.map((s) => ({
          name: basename(s.replace(/\/+$/, "")),
          dest,
          folder: target.length > 0 || dest.endsWith("/"),
        })),
      );
    }
  } else if (WRITERS.has(name)) {
    effects.writes.push(...ops);
    if (name === "tar" || name === "bsdtar") {
      trees(ops);
      // `tar xzf`, the old spelling, names its mode with no dash.
      const mode = /^[A-Za-z]+$/.test(args[0] ?? "") ? (args[0] ?? "") : "";
      if (short("x") || long("--extract", "--get") || mode.includes("x")) {
        unpack(values("C", ["--directory"]));
        // `-P` keeps a name's leading `/`, and a name rewrite (GNU tar's `--transform`, bsdtar's
        // `-s`) puts a name wherever it says.
        if (
          short("P") ||
          mode.includes("P") ||
          long("--absolute-names", "--transform", "--xform") ||
          (name === "bsdtar" && short("s"))
        ) {
          unpackAnywhere();
        }
      }
    }
  } else if (COPIERS.has(name)) {
    const last = ops.length >= 2 ? ops.slice(-1) : [];
    effects.writes.push(
      ...target,
      ...(name === "install" && short("d") ? ops : last),
    );
    if (
      name === "rsync" ||
      name === "ditto" ||
      (name === "cp" && (short("rRa") || long("--recursive", "--archive"))) ||
      (name === "scp" && short("r")) ||
      (name === "ln" && (short("s") || long("--symbolic")))
    ) {
      trees(ops);
    }
  } else if (IN_PLACE.has(name)) {
    if (short("i") || long("--in-place")) {
      // A script names no file it writes: `sed -i 's/^v = .*/v = 1/' f` writes only `f`, and
      // `ruby -C DIR -pi` writes it in DIR.
      const { notFiles, dir } = inPlaceScripts(
        name,
        args,
        joined.slice(start + 1),
      );
      effects.writes.push(
        ...args
          .filter((a, k) => !a.startsWith("-") && !notFiles.has(k))
          .map((f) =>
            dir === undefined || /^[/~]/.test(f) ? f : `${dir}/${f}`,
          ),
      );
    }
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
    const into = values("d", []);
    effects.writes.push(...into);
    unpack(into);
    // `-:` keeps a name's leading `/` and its `../`.
    if (short(":")) unpackAnywhere();
  } else if (SEVEN_ZIP.has(name)) {
    const mode = ops[0] ?? "";
    if (mode === "a" || mode === "u") {
      effects.writes.push(...ops.slice(1, 2));
      trees(ops.slice(2));
    } else if (mode === "x" || mode === "e") {
      unpack(values("o", []));
    }
  } else if (name === "cpio") {
    // cpio keeps an absolute name unless told not to.
    if (short("i")) unpack([]);
    if (short("i") && !long("--no-absolute-filenames")) unpackAnywhere();
    if (short("p")) unpack(ops.slice(-1));
  } else if (name === "pax") {
    trees(ops);
    if (short("r")) unpack(short("w") ? ops.slice(-1) : []);
    // pax keeps an absolute name, and `-s` rewrites names to anywhere.
    if (short("r") && !short("w")) unpackAnywhere();
  } else if (COMPRESSORS.has(name)) {
    if (short("r") || long("--recursive")) trees(ops);
  } else if (name === "zip") {
    effects.writes.push(...ops.slice(0, 1));
    if (short("rR") || long("--recurse-paths")) trees(ops);
  } else if (name === "find") {
    const expression = args.findIndex(
      (a) => a.startsWith("-") || a === "!" || a === "(" || a === LITERAL,
    );
    const paths = expression < 0 ? args : args.slice(0, expression);
    const from = paths.length > 0 ? paths : ["."];
    const expr = expression < 0 ? [] : args.slice(expression);
    if (piped || args.some((a) => FIND_ACTIONS.has(a))) trees(from);
    if (args.includes("-delete")) effects.writes.push(...from);
    // security-r3-4: what a find deletes or moves away may be Planroom's state, by its selection.
    if (args.includes("-delete") || findDisposes(expr)) {
      effects.finds.push({ roots: from, selects: findSelection(expr) });
    }
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
  } else if (name === "git") {
    const { dir, sub, rest } = gitParts(args);
    if (sub === "clean") {
      // `.planroom/` ignores itself, so `git clean -x` deletes it, the store with it, unless `-e`
      // keeps it; `-X` deletes only ignored files, `-e` patterns among them. `-n` deletes nothing.
      const kept =
        !short("X") &&
        values("e", ["--exclude"]).some((e) => /^\/?\.planroom\/?$/.test(e));
      if (short("d") && short("xX") && !kept && !dryRun(rest)) {
        effects.refusal =
          "git clean -x deletes the ignored .planroom/, Planroom's store with it: leave out -x, or add -e .planroom.";
      }
    } else if (sub === "stash") {
      // `--all` stashes ignored files too, and removes them from the tree: `.planroom/` among them.
      const all =
        rest.includes("--all") || rest.some((a) => /^-[^-]*a/.test(a));
      const kept = rest.some((a) =>
        /^:(!|\^|\(exclude\))\/?\.planroom\/?$/.test(a),
      );
      if (all && !kept) {
        effects.refusal =
          "git stash --all stashes and removes the ignored .planroom/, Planroom's store with it: leave out --all, or add -- ':!.planroom'.";
      }
    } else if (
      sub === "grep" &&
      (rest.includes("--no-index") || rest.includes("--untracked"))
    ) {
      // tests-3-3: outside the index, git grep reads whole trees as grep -r does.
      const given = rest.some(
        (a) => a === "-e" || a === "-f" || a.startsWith("--file"),
      );
      const positional = rest.filter((a) => !a.startsWith("-"));
      const named = given ? positional : positional.slice(1);
      trees(
        (named.length > 0 ? named : ["."]).map((r) =>
          dir === undefined || /^[/~]/.test(r) ? r : `${dir}/${r}`,
        ),
      );
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
 * What stands for an operator the second reading takes as text: one quoted, or escaped, such as
 * the `>` in `grep -r key '>' DIR`. `find` reads it as a parenthesis, which is all it can be there.
 */
const LITERAL = "§";

/** The characters the first reading splits a line on. */
const OPERATORS = ";|&<>()\n";

/** What the second reading writes for a command substitution: a variable no line sets. */
const SUBSTITUTED = "${__}";

/**
 * A Bash line read the second way (DR-86's quoted redirection; security-r2-3). The first reading
 * splits on every operator, quoted or not. This one keeps an operator inside quotes or after a
 * backslash as text, and reads each command substitution, `$(...)` or backticks, as an unknown
 * variable, so `/proc/$(pgrep planroom)/environ` reads as any process's environment, and two of
 * them in one name may be a hidden one, never `**`. The guard checks both readings and denies
 * what either denies.
 */
export function literalReading(command: string): string {
  let out = "";
  let quote: "'" | '"' | null = null;
  for (let i = 0; i < command.length; i++) {
    const ch = command.charAt(i);
    if (quote === "'") {
      if (ch === "'") quote = null;
      out += OPERATORS.includes(ch) ? LITERAL : ch;
      continue;
    }
    if (ch === "\\" && i + 1 < command.length) {
      const next = command.charAt(i + 1);
      out += OPERATORS.includes(next) ? LITERAL : ch + next;
      i++;
      continue;
    }
    if (ch === "$" && command.charAt(i + 1) === "(") {
      let depth = 0;
      let j = i + 1;
      for (; j < command.length; j++) {
        const c = command.charAt(j);
        if (c === "(") depth++;
        else if (c === ")" && --depth === 0) break;
      }
      out += SUBSTITUTED;
      i = j;
      continue;
    }
    if (ch === "`") {
      const end = command.indexOf("`", i + 1);
      out += SUBSTITUTED;
      i = end < 0 ? command.length : end;
      continue;
    }
    if (quote === '"') {
      if (ch === '"') quote = null;
      out += OPERATORS.includes(ch) ? LITERAL : ch;
      continue;
    }
    if (ch === "'" || ch === '"') quote = ch;
    out += ch;
  }
  return out;
}

/**
 * A Bash line with each `${X:-word}`, `${X-word}`, `${X:=word}`, `${X=word}` and `${X:+word}` as
 * its `word`, and each `${X/pat/rep}` (`//`, `/#`, `/%`) as its `rep`, innermost first: the other
 * reading puts an unknown `X` in as any name, but the shell puts in `word` when `X` is unset, so
 * `${A:-.planroom}` may be the state directory, and `${PWD/*\/.planroom}` is it.
 */
function withDefaults(command: string): string {
  const word = "((?:[^{}$]|\\$(?!\\{))*)";
  const defaults = new RegExp(
    `\\$\\{[A-Za-z_][A-Za-z0-9_]*:?[-=+]${word}\\}`,
    "g",
  );
  const swaps = new RegExp(
    `\\$\\{[A-Za-z_][A-Za-z0-9_]*\\/[/#%]?(?:\\\\.|[^/}\\\\])*\\/${word}\\}`,
    "g",
  );
  let out = command;
  for (let k = 0; k < 16; k++) {
    const next = out.replace(defaults, "$1").replace(swaps, "$1");
    if (next === out) break;
    out = next;
  }
  return out;
}

/**
 * The line as written, and the other ways the guard reads it, each once: with every
 * backslash-newline a quoted `sh -c` script drops dropped (the first reading keeps one inside
 * single quotes, as Bash does), with each one as a plain newline (a comment ends there, so
 * `true #\` then a newline hides nothing), with each expansion's default or replacement put in,
 * and each of those as the second reading takes it.
 */
function readings(command: string): string[] {
  const forms = new Set<string>();
  for (const line of [
    command,
    joinLines(command),
    command.replace(/\\\n/g, "\n"),
  ]) {
    for (const form of [line, withDefaults(line)]) {
      forms.add(form);
      forms.add(literalReading(form));
    }
  }
  return [...forms];
}

/** `shopt -s dotglob`, zsh's `setopt globdots` or a set `GLOBIGNORE`: `*` then matches hidden names. */
const DOTGLOB =
  /\b(shopt|setopt)\b[^;&|\n]*\b(dotglob|glob_?dots)\b|\bGLOBIGNORE\s*=/i;
/** zsh's `D` glob qualifier, as in `~/*(D)`, which matches hidden names for that one glob. */
const DOT_QUALIFIER = /[*?\]]\([^)]*D[^)]*\)/;

/** A glob whose segments, `**` among them, may match a leading dot, as they do once dotglob is on. */
function withDots(word: string): string {
  if (!GLOB.test(word)) return word;
  return word
    .split("/")
    .map((s) => (/^[*?[]/.test(s) ? `{,}${s}` : s))
    .join("/");
}

/** A `file:` URL as the path it names, or the string unchanged. */
function fromFileUrl(word: string): string {
  const url = /^file:\/\/(?:localhost)?(\/.*)$/i.exec(word);
  if (url === null) return word;
  try {
    return decodeURIComponent(url[1] ?? "");
  } catch {
    return url[1] ?? "";
  }
}

/**
 * Where a Bash line's words may land: each word that may name a file, every `cd` target, and what
 * its commands write, delete, unpack and read as whole trees. Every word an effect names is also
 * a word, so the caller resolves it.
 */
export function bashPaths(command: string, places: GuardPlaces): BashPaths {
  const vars = knownVariables(places);
  const commands = shellCommands(command);
  const piped = commands.some((c) =>
    c.words.some((w) => basename(w) === "xargs"),
  );
  const dotted = DOTGLOB.test(unquote(command)) || DOT_QUALIFIER.test(command);
  /** A word as the guard checks it: a file URL as its path, a glob under dotglob as matching dot names. */
  const shown = (w: string): string => {
    const path = fromFileUrl(w);
    return dotted ? withDots(path) : path;
  };
  const candidates: string[] = [];
  const effects = noEffects();
  let bases = [places.cwd];
  const cds: string[] = [];
  // A shell outside the workspace's root, left there by an earlier call's `cd`, may sit where a
  // bare name is a guarded file (`cat config.toml`), so every word counts as a path (security-r3-1).
  let moved = places.cwd !== places.workspace;
  /** Each simple command's words, expanded. */
  const expanded: string[][] = [];
  /** A `cd` to `target` (expanded), from every place the line may be by then. */
  const moveTo = (target: string): void => {
    cds.push(target);
    const moves = bases.map((b) => normalize(target, b, places.home));
    bases = [...new Set([...bases, ...moves])].slice(0, 8);
    moved = true;
  };
  /**
   * Whether the command substitution that starts after command `k` gives its command a word to
   * print (`$(echo .config)`), rather than printing what the line doesn't spell (`$(mktemp -d)`).
   */
  const substitutionNames = (k: number): boolean => {
    for (let j = k + 1; j < commands.length; j++) {
      const words = commands[j]?.words ?? [];
      if (
        words.slice(commandStart(words) + 1).some((w) => !w.startsWith("-"))
      ) {
        return true;
      }
      const end = commands[j]?.sep;
      if (end === ")" || end === "`") break;
    }
    return false;
  };
  for (const [k, simple] of commands.entries()) {
    const words: string[] = [];
    const values: (string | null)[] = [];
    const assignments: boolean[] = [];
    for (const [i, raw] of simple.words.entries()) {
      // `NAME=value` and `--flag=value`: the value may be a path, and a name the line sets is known.
      const eq = raw.indexOf("=");
      // `NAME+=value` appends: `X=.plan; X+=room` names `.planroom`, and an append to a name the
      // line doesn't know joins an unknown to the value.
      const appends = eq > 1 && raw.charAt(eq - 1) === "+";
      const name = raw.slice(0, appends ? eq - 1 : eq);
      const assigned = eq > 0 && /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
      const value =
        eq < 0
          ? null
          : shown(
              expand(
                (assigned && appends ? `\${${name}}` : "") + raw.slice(eq + 1),
                vars,
              ),
            );
      // `A=$(echo .config)`: what the substitution prints may be any name, a hidden one too.
      const substituted =
        assigned &&
        i === simple.words.length - 1 &&
        ((simple.sep === "(" && raw.endsWith("$")) || simple.sep === "`");
      if (assigned && value !== null) {
        vars.set(
          name,
          !substituted
            ? value
            : substitutionNames(k)
              ? ANY_JOINED
              : `${value}*`.replace(/\*+$/, "*"),
        );
      }
      words.push(shown(expand(raw, vars)));
      values.push(value);
      assignments.push(assigned);
      if (raw === "cd" || raw === "pushd") {
        const next = simple.words[i + 1];
        moveTo(
          next === undefined || next.startsWith("-")
            ? "~"
            : shown(expand(next, vars)),
        );
      }
    }
    // Names the command sets from what the line doesn't spell (`read`, `mapfile`, `printf -v`)
    // may be any name, a hidden one too; a `for` loop's from its list.
    const ident = /^[A-Za-z_][A-Za-z0-9_]*$/;
    const head = simple.words[0] ?? "";
    const loopVar = simple.words[1] ?? "";
    if (head === "for" && ident.test(loopVar)) {
      const list = simple.words[2] === "in" ? words.slice(3) : [];
      vars.set(
        loopVar,
        list.length === 1
          ? (list[0] ?? ANY_JOINED)
          : list.length > 1 && !list.some((w) => /[/{},]/.test(w))
            ? `{${list.join(",")}}`
            : ANY_JOINED,
      );
    }
    const at = commandStart(simple.words);
    const program = basename(simple.words[at] ?? "");
    const after = simple.words.slice(at + 1);
    if (["read", "mapfile", "readarray"].includes(program)) {
      for (const w of after) if (ident.test(w)) vars.set(w, ANY_JOINED);
    }
    if (program === "printf") {
      after.forEach((w, j) => {
        const name = w === "-v" ? after[j + 1] : /^-v(.+)$/.exec(w)?.[1];
        if (name !== undefined && ident.test(name)) vars.set(name, ANY_JOINED);
      });
    }
    // `env -C DIR`, `sudo -D DIR`: the command runs there, as after a `cd`.
    for (const dir of commandParts(simple.words).dirs) {
      moveTo(shown(expand(dir, vars)));
    }
    // A find's excluding pattern (`-not -path '*/.planroom/*'`) keeps a place out; it names none.
    const finding = basename(words[commandStart(words)] ?? "") === "find";
    words.forEach((word, i) => {
      const value = values[i] ?? null;
      if (value !== null && value !== "") candidates.push(value);
      const excluding =
        finding &&
        FIND_TESTS.has(words[i - 1] ?? "") &&
        (words[i - 2] === "!" || words[i - 2] === "-not");
      if (assignments[i] !== true && !word.startsWith("-") && !excluding) {
        candidates.push(word);
      }
    });
    const writes = simple.writes.map((w) => shown(expand(w, vars)));
    candidates.push(
      ...writes,
      ...simple.reads.map((w) => shown(expand(w, vars))),
    );
    const own = commandEffects(words, piped, simple.joined);
    effects.writes.push(...writes, ...own.writes);
    effects.removes.push(...own.removes);
    effects.trees.push(...own.trees);
    effects.unpacks.push(...own.unpacks);
    effects.arrivals.push(...own.arrivals);
    effects.finds.push(...own.finds);
    effects.refusal ??= own.refusal;
    if (commandParts(simple.words).attachedSplit) {
      effects.question ??= SPLIT_ATTACHED;
    }
    expanded.push(words);
  }
  /** The commands whose output a command substitution right after command `k` hands it. */
  const substituted = (k: number): number[] => {
    const sep = commands[k]?.sep;
    const out: number[] = [];
    if (sep !== "(" && sep !== "`") return out;
    for (let j = k + 1; j < commands.length; j++) {
      out.push(j);
      const end = commands[j]?.sep;
      if (end === ")" || end === "`") break;
    }
    return out;
  };
  // A `cd $(...)` may go wherever the substitution's own words point: `cd $(echo ~/.config/planroom)`.
  commands.forEach((simple, k) => {
    const start = commandStart(simple.words);
    const program = simple.words[start];
    if (program !== "cd" && program !== "pushd") return;
    for (const j of substituted(k)) {
      const words = expanded[j] ?? [];
      const targets = words
        .slice(commandStart(words) + 1)
        .filter((w) => !w.startsWith("-"));
      cds.push(...targets);
      const moves = targets.flatMap((t) =>
        bases.map((b) => normalize(t, b, places.home)),
      );
      bases = [...new Set([...bases, ...moves])].slice(0, 16);
    }
  });
  // security-r3-4: an `rm` or `mv` fed by xargs, or by a command substitution (`rm -rf $(echo
  // .planroom)`), disposes of what the commands feeding it name, or of what a feeding find selects.
  commands.forEach((simple, k) => {
    const start = commandStart(simple.words);
    if (!DISPOSERS.has(basename(simple.words[start] ?? ""))) return;
    const feeders: number[] = substituted(k);
    if (simple.words.slice(0, start).some((w) => FEEDERS.has(basename(w)))) {
      for (let j = k - 1; j >= 0 && commands[j]?.sep === "|"; j--) {
        feeders.push(j);
      }
    }
    for (const j of feeders) {
      const words = expanded[j] ?? [];
      const at = commandStart(words);
      const args = words.slice(at + 1);
      if (basename(words[at] ?? "") === "find") {
        const expression = args.findIndex(
          (a) => a.startsWith("-") || a === "!" || a === "(" || a === LITERAL,
        );
        const paths = expression < 0 ? args : args.slice(0, expression);
        effects.finds.push({
          roots: paths.length > 0 ? paths : ["."],
          selects: findSelection(expression < 0 ? [] : args.slice(expression)),
        });
      } else {
        const named = args.filter((a) => !a.startsWith("-"));
        effects.writes.push(...named);
        effects.removes.push(...named);
      }
    }
  });
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
    ...effects.unpacks,
    ...effects.arrivals.map((a) => a.dest),
    ...effects.finds.flatMap((f) => f.roots),
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

/**
 * Whether the glob segment `segment` may match the name `name`. A `**` matches one name the way
 * `*` does, never a hidden one unless dotglob spelled it `{,}**`; `mayName` reads it as folders.
 */
function segmentMatches(segment: string, name: string): boolean {
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
 * Whether a name, maybe a glob, singles out a file Planroom keeps in a state directory: it may be
 * one, and its stem, before the first `.`, is no bare wildcard (`*`, `**` or the dotglob form),
 * as `*.db` and `*.json` are, which match any name of that kind.
 */
function namesStateFile(segment: string): boolean {
  const stem = segment.split(".")[0] ?? "";
  if (["*", "**", "{,}*", DOT_GLOBSTAR, ANY_JOINED].includes(stem)) {
    return false;
  }
  return STATE_SECRETS.some((n) => segmentMatches(segment, n));
}

/**
 * Why a glob (absolute and normal) may expand into a place an agent may not read, or null: it
 * names one already; it lies inside a guarded directory; its literal prefix holds one that its
 * segments, a `**` as any number of unhidden folders, can reach; or it may name a store or serve
 * file inside a state directory.
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
  const data = places.dataRoot;
  const guarded = [
    ...DATA_SECRETS.map((s) => `${data}/${s}`),
    places.userConfig,
    modStoreDir(places),
  ];
  for (const dir of guarded) {
    if (within(prefix, dir) || mayName(pattern, dir, "within")) {
      return (
        readDenied(dir, places) ??
        "Planroom's private files are read through the planroom CLI, never directly."
      );
    }
  }
  // A store or serve file sits in a state directory: `.planroom/`, or `planroom/` in a git directory.
  const folders = segments.slice(0, -1);
  const last = segments.at(-1) ?? "";
  const stateFolder =
    inStateDir(prefix, places) ||
    stateDirEnds(folders, segmentMatches).length > 0 ||
    namedGitDirs(places).some(({ state }) =>
      mayName(`/${folders.join("/")}`, state, "within"),
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
  /**
   * For a tool the guard doesn't know: each string that may be a path but reads like text, such as
   * one with a space in it, checked only where the file system says it lands.
   */
  linked?: string[];
  /** Whether the tool is one the guard doesn't know, whose strings are all it can read. */
  unknown?: boolean;
  /** Whether the input holds more strings, or deeper ones, than the guard reads. */
  unread?: boolean;
}

/** The most strings, and the deepest nesting, the guard reads in an unknown tool's input. */
const STRING_LIMIT = 1024;
const DEPTH_LIMIT = 32;

/**
 * The most paths the caller resolves for one call (security-r2-4): past it, the guard asks rather
 * than check a path it never resolved.
 */
export const RESOLVE_LIMIT = 1024;

/**
 * A string an unknown tool may take as a file, as the path to check: a `file:` URL as its path; any
 * other URL, an empty, multi-line or very long string, never.
 */
function asPath(value: string): string | null {
  if (value === "" || value.includes("\n") || value.length > 4096) return null;
  if (/^file:/i.test(value)) return fromFileUrl(value);
  return /^[a-z][a-z0-9+.-]*:\/\//i.test(value) ? null : value;
}

/** Whether a path-like string reads as a path as written: no space, or one that starts like a path. */
function pathShaped(value: string): boolean {
  return /^(\/|~|\.\.?\/)/.test(value) || !/\s/.test(value);
}

/** Every string an input holds, at any depth, keys left out, and whether there were more than the guard reads. */
function stringsIn(
  value: unknown,
  depth = 0,
  out: { strings: string[]; over: boolean } = { strings: [], over: false },
): { strings: string[]; over: boolean } {
  if (out.strings.length >= STRING_LIMIT || depth > DEPTH_LIMIT) {
    out.over = true;
    return out;
  }
  if (typeof value === "string") out.strings.push(value);
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
 * holds held to the read rule, and to the write rule unless the tool says it only reads.
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
    default: {
      // A tool the guard doesn't know: every string it holds may be a path (security-r3-2).
      const found = stringsIn(input);
      const paths = found.strings
        .map(asPath)
        .filter((p): p is string => p !== null);
      return {
        ...none,
        reads: paths.filter(pathShaped),
        linked: paths.filter((p) => !pathShaped(p)),
        unknown: true,
        unread: found.over,
      };
    }
  }
}

/**
 * Every path a call names, which the caller resolves before it asks `guardCall`: a Bash line's
 * words in both its readings. The caller asks rather than resolve more than `RESOLVE_LIMIT`.
 */
export function namedPaths(
  tool: string,
  input: unknown,
  places: GuardPlaces,
): string[] {
  const { reads, writes, uploads, bash, linked } = callPaths(tool, input);
  const fromBash =
    bash === null
      ? []
      : readings(bash).flatMap((line) => bashPaths(line, places).words);
  return [
    ...new Set([
      ...reads,
      ...(linked ?? []),
      ...writes,
      ...uploads,
      ...fromBash,
    ]),
  ];
}

/** One place a `find` under some root may reach, whether it's a folder, and whether it may be empty. */
interface Sample {
  path: string;
  dir: boolean;
  empty: boolean;
}

/**
 * The files Planroom keeps that may be empty: a WAL a checkpoint truncated, a rollback journal,
 * and the data root's lock, which `planroom` creates and never writes (app/data.rs `lock`).
 */
const EMPTY_STATE = ["store.db-wal", "store.db-journal"];

/**
 * The protected places at or below `root` (absolute and normal) that a deleting `find` there must
 * not select: the workspace's state directory and its store, a repo's `.git` with the state it and
 * its linked worktrees' and submodules' git directories may hold, the state directory in the git
 * directory a `.git` file names, the data root and its engine, the mod's store, the user config
 * and the plugin's folders; and, under a root at or above the home folder, another workspace's
 * state directory.
 */
function protectedSamples(root: string, places: GuardPlaces): Sample[] {
  const out: Sample[] = [];
  const add = (path: string, dir: boolean, empty = false): void => {
    if (path !== root && within(path, root)) out.push({ path, dir, empty });
  };
  /** A folder that holds a store, its files with data in them and those that may be empty. */
  const store = (folder: string, names: readonly string[]): void => {
    add(folder, true);
    for (const n of names) add(`${folder}/${n}`, false);
    for (const n of EMPTY_STATE) add(`${folder}/${n}`, false, true);
  };
  const state = (w: string): void => {
    add(`${w}/.git`, true);
    // A linked worktree's git directory and a submodule's, each with its own state (DR-74).
    for (const git of ["worktrees", "worktrees/wt", "modules", "modules/lib"]) {
      add(`${w}/.git/${git}`, true);
    }
    for (const folder of [
      `${w}/.planroom`,
      `${w}/.git/planroom`,
      `${w}/.git/worktrees/wt/planroom`,
      `${w}/.git/modules/lib/planroom`,
    ]) {
      store(folder, [...STATE_SECRETS, "config.toml"]);
    }
  };
  for (const w of new Set([places.workspace, places.cwd])) {
    state(w);
    // Rooted deeper in its `.git`, such as at `.git/modules/vendor`, a find reaches the state
    // directory of any git directory below.
    if (within(root, `${w}/.git`)) {
      for (const more of ["planroom", "wt/planroom"]) {
        const folder = `${root.replace(/\/+$/, "")}/${more}`;
        const p = segmentsOf(folder);
        if (stateDirEnds(p, same).includes(p.length - 1)) {
          store(folder, [...STATE_SECRETS, "config.toml"]);
        }
      }
    }
  }
  // The git directory a `.git` file names, wherever that is, and the state directory in it.
  for (const { gitDir, state } of namedGitDirs(places)) {
    add(gitDir, true);
    store(state, [...STATE_SECRETS, "config.toml"]);
  }
  if (within(places.home, root)) state(`${root.replace(/\/+$/, "")}/project`);
  const data = places.dataRoot;
  add(data, true);
  for (const n of ["stores", "backups", "open", "engine", "logs"]) {
    add(`${data}/${n}`, true);
  }
  store(`${data}/stores/0`, ["store.db"]);
  for (const n of ["ledger.db", "workspaces.json", "engine/0/planroom"]) {
    add(`${data}/${n}`, false);
  }
  add(`${data}/workspaces.json.lock`, false, true);
  add(modStoreDir(places), true);
  add(`${modStoreDir(places)}/planroom_planroom-0.json`, false);
  add(places.userConfig, false);
  for (const p of pluginFolders(places)) {
    add(p, true);
    add(`${p}/hooks/planroom-mod.js`, false);
  }
  for (const f of hostPluginLists(places)) add(f, false);
  return out;
}

/** One name a glob segment (not `**`) may stand for, or null when the guard can't spell one. */
function exampleName(segment: string): string | null {
  let out = "";
  for (let i = 0; i < segment.length; i++) {
    const ch = segment[i] ?? "";
    if (ch === "*") continue;
    if (ch === "?") out += "x";
    else if (ch === "[" && segment.indexOf("]", i + 2) > 0) {
      const end = segment.indexOf("]", i + 2);
      const set = segment.slice(i + 1, end);
      out += /^[!^]/.test(set) ? "x" : set.charAt(0);
      i = end;
    } else if (ch === "{" && segment.indexOf("}", i) > 0) {
      const end = segment.indexOf("}", i);
      out += exampleName(segment.slice(i + 1, end).split(",")[0] ?? "") ?? "";
      i = end;
    } else out += ch;
  }
  const name = out === "" ? "x" : out;
  return segmentMatches(segment, name) ? name : null;
}

/**
 * The folders a `find` root that holds a glob (absolute and normal) may name, as the shell
 * expands it before find runs, read as rm's operands are: each protected place below its literal
 * folder, and each folder above one, that the glob may name; and the glob with each segment that
 * can't be a protected place's name spelled as some name it may stand for, such as a worktree's.
 * A root with no literal folder written before its glob (`**`, `*`) never names the folder it
 * runs in.
 */
function globRoots(
  root: string,
  written: string,
  places: GuardPlaces,
): string[] {
  const p = segmentsOf(root);
  const first = p.findIndex((s) => GLOB.test(s));
  const base = `/${p.slice(0, first).join("/")}`;
  const out = new Set<string>();
  for (const sample of [
    base,
    ...protectedSamples(base, places).map((s) => s.path),
  ]) {
    const s = segmentsOf(sample);
    for (let n = first; n <= s.length; n++) {
      const folder = `/${s.slice(0, n).join("/")}`;
      if (mayName(root, folder, "is")) out.add(folder);
    }
    if (!p.some(isGlobstar)) {
      const names = p.map((segment, i) =>
        i < first || segmentMay(segment, s[i] ?? "")
          ? (s[i] ?? segment)
          : exampleName(segment),
      );
      if (names.every((n) => n !== null)) out.add(`/${names.join("/")}`);
    }
  }
  const bare =
    !written.startsWith("/") && GLOB.test(written.split("/")[0] ?? "");
  return [...out].filter((f) => !bare || f !== base);
}

/**
 * How a find spells `folder`, one of the folders its root `written` (as written) may name when
 * the shell expands it: the root's literal folder as written, then the folder's names below it.
 */
function globWritten(written: string, root: string, folder: string): string {
  const w = written.split("/");
  const prefix = w
    .slice(
      0,
      w.findIndex((s) => GLOB.test(s)),
    )
    .join("/");
  const literal = segmentsOf(root).findIndex((s) => GLOB.test(s));
  const tail = segmentsOf(folder).slice(literal).join("/");
  if (prefix === "") {
    return written.startsWith("/") ? `/${tail}` : tail === "" ? "." : tail;
  }
  return tail === "" ? prefix : `${prefix}/${tail}`;
}

const FIND_DELETE =
  "This find deletes or moves what it selects, which may include Planroom's state, its data root, the person key, the gateway key or the plugin. Keep them out, such as with -not -path '*/.planroom/*' -not -path '*/.git/*', or name a folder that holds none of them.";

/** A guarded variable named anywhere: a start-up or settings file only sets it. */
const GUARDED_NAME = new RegExp(
  `(?<![A-Za-z0-9_])(${GUARDED_ALTERNATIVES})(?![A-Za-z0-9_])`,
);

/**
 * A place variable named as a setting names it (`HOME=`, `export HOME`, `"HOME":`, and
 * `${HOME:=x}` or `${HOME=x}`, which assign), not as a line uses it (`$HOME`, `${HOME}`,
 * `${HOME:-x}`), which start-up files do every day.
 */
const PLACE_NAME = new RegExp(
  [
    `(?<![A-Za-z0-9_$])(?<!\\$\\{)(${PLACE_VARIABLES.join("|")})(?![A-Za-z0-9_])`,
    `\\$\\{(${PLACE_VARIABLES.join("|")}):?=`,
  ].join("|"),
);

const STARTUP_SET = `An agent never sets ${spelled(GUARDED_VARIABLES)}, in a shell's start-up file or Claude Code's settings either, nor ${spelled(PLACE_VARIABLES)} there, which place the engine cache a press runs.`;

/**
 * Whether text names a variable no start-up or settings file an agent writes sets, its quotes
 * taken out and JSON's `\uXXXX` escapes put in.
 */
function namesGuardedVariable(text: string): boolean {
  const decoded = text.replace(/\\u([0-9a-fA-F]{4})/g, (_, hex: string) =>
    String.fromCharCode(parseInt(hex, 16)),
  );
  return [text, decoded].some((t) => {
    const plain = unquote(t);
    return GUARDED_NAME.test(plain) || PLACE_NAME.test(plain);
  });
}

/** A shell's start-up files in the home folder; zsh's also in `$ZDOTDIR`. */
const STARTUP_FILES = [
  ".bashrc",
  ".bash_profile",
  ".bash_login",
  ".profile",
  ".zshrc",
  ".zshenv",
  ".zprofile",
  ".zlogin",
];

/**
 * The files, and the folders whose files (by suffix), that set the environment of a later shell
 * or session for these places: a shell's start-up files, `~/.bashrc.d/`, fish's config, systemd's
 * `environment.d/`, and Claude Code's user settings. Its project and managed settings, which may
 * be anywhere, `setsEnvironment` knows by name.
 */
function environmentFiles(places: GuardPlaces): {
  files: string[];
  folders: { dir: string; suffix: string }[];
} {
  const home = places.home.replace(/\/+$/, "");
  const files = STARTUP_FILES.map((n) => `${home}/${n}`);
  if (places.zdotdir !== undefined) {
    files.push(
      ...STARTUP_FILES.filter((n) => n.startsWith(".z")).map(
        (n) => `${places.zdotdir ?? ""}/${n}`,
      ),
    );
  }
  const folders = [{ dir: `${home}/.bashrc.d`, suffix: "" }];
  // `${XDG_CONFIG_HOME:-$HOME/.config}`, the folder above the user config's, and the default.
  const xdg = places.userConfig.split("/").slice(0, -2).join("/");
  for (const config of new Set([xdg, `${home}/.config`])) {
    files.push(`${config}/fish/config.fish`);
    folders.push(
      { dir: `${config}/fish/conf.d`, suffix: ".fish" },
      { dir: `${config}/environment.d`, suffix: ".conf" },
    );
  }
  files.push(
    `${places.claudeConfig}/settings.json`,
    `${places.claudeConfig}/settings.local.json`,
  );
  return { files, folders };
}

/**
 * Whether a file (absolute and normal, maybe a glob) sets the environment of a later shell or
 * session: a start-up file, or a Claude Code settings file, whose `env` every session and its
 * tools inherit. A glob counts when it may name one, as the guard's other globs do.
 */
function setsEnvironment(path: string, places: GuardPlaces): boolean {
  const { files, folders } = environmentFiles(places);
  const p = segmentsOf(path);
  const last = p.at(-1) ?? "";
  const parent = p.at(-2) ?? "";
  // Project and local settings, wherever they are: a glob's folder must name `.claude`.
  if (
    segmentMay(parent, ".claude") &&
    ["settings.json", "settings.local.json"].some((n) => segmentMay(last, n))
  ) {
    return true;
  }
  if (!GLOB.test(path)) {
    return (
      files.includes(path) ||
      folders.some(
        (f) => path !== f.dir && within(path, f.dir) && path.endsWith(f.suffix),
      ) ||
      // Managed settings by name, wherever they are.
      last === "managed-settings.json" ||
      parent === "managed-settings.d"
    );
  }
  // A glob may name a managed settings file only along its full path, else `out/*.json` would.
  return (
    [...files, ...MANAGED_SETTINGS].some((f) => mayName(path, f, "within")) ||
    [
      ...folders.map((f) => f.dir),
      ...MANAGED_SETTINGS.map((f) => f.replace(/\.json$/, ".d")),
    ].some((d) => mayName(path, d, "within"))
  );
}

/** Where Claude Code keeps managed settings: Linux, then macOS. */
const MANAGED_SETTINGS = [
  "/etc/claude-code/managed-settings.json",
  "/Library/Application Support/ClaudeCode/managed-settings.json",
];

/** The text a Write, Edit or MultiEdit puts in its file, or null for another tool. */
function writtenText(tool: string, input: unknown): string | null {
  const i = (
    typeof input === "object" && input !== null ? input : {}
  ) as Record<string, unknown>;
  const text = (v: unknown): string => (typeof v === "string" ? v : "");
  if (tool === "Write") return text(i["content"]);
  if (tool === "Edit") return text(i["new_string"]);
  if (tool === "MultiEdit") {
    const edits = Array.isArray(i["edits"]) ? (i["edits"] as unknown[]) : [];
    return edits
      .map((e) =>
        typeof e === "object" && e !== null
          ? text((e as Record<string, unknown>)["new_string"])
          : "",
      )
      .join("\n");
  }
  return null;
}

/**
 * Why the person decides a call the guard would let through, or null: it runs on another attached
 * machine (`_host`), whose folders the guard can't place, or outside the sandbox.
 */
function elsewhere(input: unknown): string | null {
  if (typeof input !== "object" || input === null) return null;
  const i = input as Record<string, unknown>;
  if (i["_host"] !== undefined && i["_host"] !== null) {
    return "This call runs on another attached machine, where the Planroom guard can't tell where Planroom's files are, so it can't check what the call reaches.";
  }
  // Any value the host may read as true: `"true"` as well as `true`.
  const sandbox = i["dangerouslyDisableSandbox"];
  const inside =
    sandbox === undefined ||
    sandbox === null ||
    sandbox === false ||
    sandbox === 0 ||
    sandbox === "";
  if (!inside) {
    return "This command asks to run outside the sandbox, so you decide whether it runs.";
  }
  return null;
}

/**
 * The verdict on one call, given every spelling of each path it names: its lexical form and,
 * where the file system resolved it, where it lands. `readOnly` is the host's word that the tool
 * only reads this input. Null lets the call go on to the session's own permission rules. A call
 * on another attached machine, or outside the sandbox, the guard would let through is asked.
 */
export function guardCall(
  tool: string,
  input: unknown,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
  readOnly = false,
): Verdict | null {
  // tests-3-1: with no home folder to place them, the guard can't tell what a call reaches.
  if (places.unplaced !== undefined) {
    return { decision: "ask", reason: places.unplaced };
  }
  if (places.implementer !== undefined) {
    const own = implementerVerdict(
      tool,
      input,
      resolved,
      places,
      places.implementer,
    );
    if (own !== undefined) {
      if (own?.decision === "deny") return own;
      const why = elsewhere(input);
      return why === null ? own : { decision: "ask", reason: why };
    }
  }
  const verdict = localVerdict(tool, input, resolved, places, readOnly);
  if (verdict?.decision === "deny") return verdict;
  const why = elsewhere(input);
  return why === null ? verdict : { decision: "ask", reason: why };
}

/** The tools that write the file their input names. */
const WRITE_TOOLS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);

/** What the implementer runs that DR-52 names: the store's shell, a pty, a push, the hand-off. */
const IMPLEMENTER_NEVER =
  "The implementer never runs sqlite3, script, git push or planroom handoff: the hand-off's own steps do that for it.";

/** architecture.md, Actors: the implementer carries on without what it wasn't approved to run. */
const IMPLEMENTER_CARRY_ON =
  "This command isn't on the hand-off's approved list, so it doesn't run; carry on without it. The review card lists it, with a retry that allows it.";

const IMPLEMENTER_WRITE =
  "The implementer writes only inside its hand-off's clones, never in a clone's .git/, and its result.json.";

/**
 * The verdict on a spawned implementer's call, or undefined when the guard's rules for every agent
 * decide it. Its writes are allowed in its own clones, outside each clone's `.git/`, and on the
 * hand-off's `result.json`, checked before the data-root write rule, since the clones sit under
 * `DATA_ROOT/handoffs/`; every other write is denied, the workspace's included. A Read, Grep or
 * Glob that stays inside its clones is exempt from the data-root rules and goes on to the
 * session's own (null). Every Bash call is denied in M1, which approves no command yet.
 */
function implementerVerdict(
  tool: string,
  input: unknown,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
  own: { clones: readonly string[]; result: string },
): Verdict | null | undefined {
  const deny = (reason: string): Verdict => ({ decision: "deny", reason });
  const clones = own.clones.map((c) => normalize(c, "/", places.home));
  const result = normalize(own.result, "/", places.home);
  const spell = (p: string): string[] => [
    normalize(p, places.cwd, places.home),
    ...resolved(p),
  ];
  const inClone = (s: string): boolean =>
    clones.some((c) => within(s, c) && !within(s, `${c}/.git`));
  const { reads, writes, bash } = callPaths(tool, input);
  if (bash !== null) {
    const words = shellWords(unquote(bash)).map(basename);
    const never =
      words.includes("sqlite3") ||
      words.includes("script") ||
      words.some(
        (w, i) => w === "git" && words.slice(i + 1).includes("push"),
      ) ||
      words.some((w, i) => w === "planroom" && words[i + 1] === "handoff");
    return deny(never ? IMPLEMENTER_NEVER : IMPLEMENTER_CARRY_ON);
  }
  if (WRITE_TOOLS.has(tool)) {
    if (writes.length === 0) return deny(IMPLEMENTER_WRITE);
    const ok = writes.every((p) =>
      spell(p).every((s) => s === result || (!GLOB.test(s) && inClone(s))),
    );
    return ok
      ? {
          decision: "allow",
          reason: "The implementer writes in its own clone.",
        }
      : deny(IMPLEMENTER_WRITE);
  }
  if (tool === "Read" || tool === "Grep" || tool === "Glob") {
    const fields = (input ?? {}) as Record<string, unknown>;
    const rootField = fields["path"];
    const root =
      typeof rootField === "string" && rootField !== "" ? rootField : ".";
    const filter = fields[tool === "Grep" ? "glob" : "pattern"];
    const filterStays =
      typeof filter !== "string" ||
      (!filter.startsWith("/") &&
        !filter.startsWith("~") &&
        !segmentsOf(filter).includes(".."));
    const paths = tool === "Read" ? reads : [root, ...reads];
    if (
      (tool === "Read" || filterStays) &&
      paths.length > 0 &&
      paths.every((p) =>
        spell(p).every((s) => clones.some((c) => within(s, c))),
      )
    ) {
      return null;
    }
  }
  return undefined;
}

/** `guardCall`'s verdict for a call run here, on this machine, in the sandbox the session has. */
function localVerdict(
  tool: string,
  input: unknown,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
  readOnly: boolean,
): Verdict | null {
  const { reads, writes, uploads, bash, linked, unknown, unread } = callPaths(
    tool,
    input,
  );
  const deny = (reason: string): Verdict => ({ decision: "deny", reason });
  if (unread === true) {
    return {
      decision: "ask",
      reason:
        "This call's input holds more than the Planroom guard reads, so it can't check what the call reaches.",
    };
  }
  // A tool the guard doesn't know, that doesn't say it only reads, may write what it names. It may
  // as well name a path somewhere else, such as a repo path it pushes, so the person decides.
  const writesToo = unknown === true && !readOnly;
  const ask = (reason: string): Verdict => ({
    decision: "ask",
    reason: `This tool may write what it names, and ${reason.charAt(0).toLowerCase()}${reason.slice(1)}`,
  });
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
      const write = writesToo ? writeTargetDenied(s, places) : null;
      if (write !== null) return ask(write);
      // A search rooted at a state directory, the data root, or a folder that holds the mod's
      // store, the user-level config or the data root's private files reaches what's inside it.
      if (
        (tool === "Grep" || tool === "Glob") &&
        (inStateDir(s, places) ||
          within(s, places.dataRoot) ||
          secretPlaces(places).some((secret) => within(secret, s)))
      ) {
        return deny(
          "Planroom's state is read through the planroom CLI, never directly.",
        );
      }
    }
  }
  // A Grep's file filter matches at any depth below the search's root, a state directory's
  // included, as ripgrep matches a glob; a Glob's pattern is anchored at its path, where a `**`
  // still spans folders. Either names only files below its root, the call's path or else the
  // shell's directory, so a relative one is read from there: `**/*.ts` from the workspace never
  // reaches a git directory a `.git` file names elsewhere, and `.git/*` lists what `*` from
  // `.git` does.
  const fields = (input ?? {}) as Record<string, unknown>;
  const filter = fields[tool === "Grep" ? "glob" : "pattern"];
  const rootField = fields["path"];
  const root = normalize(
    typeof rootField === "string" && rootField !== "" ? rootField : ".",
    places.cwd,
    places.home,
  );
  const anchored = (f: string): string => normalize(f, root, places.home);
  // The Glob tool lists files, so its pattern's last name is a file's: it reaches a state
  // directory's files only when the folders before it may lie in one, or a `**` ends it.
  const folderOf = (f: string): string => {
    const p = segmentsOf(f);
    return isGlobstar(p.at(-1) ?? "") ? f : `/${p.slice(0, -1).join("/")}`;
  };
  const below = (f: string): string[] =>
    tool === "Glob"
      ? [folderOf(anchored(f))]
      : [anchored(f), ...(/^(\/|~)/.test(f) ? [] : [anchored(`**/${f}`)])];
  if (
    (tool === "Grep" || tool === "Glob") &&
    typeof filter === "string" &&
    (segmentsOf(filter).some(namesStateFile) ||
      // ripgrep matches a filter's folders wherever they sit below its root.
      (tool === "Grep" &&
        stateDirEnds(segmentsOf(filter), segmentMay).length > 0) ||
      below(filter).some((f) => mayBeInStateDir(f, places)))
  ) {
    return deny(
      "Planroom's store is read through the planroom CLI, never directly.",
    );
  }
  // ripgrep skips hidden folders below its root, not the root: rooted at a git directory, or a
  // folder above a worktree's or a submodule's, a Grep reads the state directory's files as
  // `rg` there does, unless its filter keeps every state file out.
  if (tool === "Grep" && typeof rootField === "string" && rootField !== "") {
    const keepsOut =
      typeof filter === "string" &&
      !filter.startsWith("!") &&
      !STATE_SECRETS.some((n) =>
        segmentMatches(segmentsOf(filter).at(-1) ?? "", n),
      );
    for (const s of spell(rootField)) {
      const reason = keepsOut ? null : treeDenied(s, false, places);
      if (reason !== null) return deny(reason);
    }
  }
  // An unknown tool's string that reads like text counts by the folder it spells and where it
  // lands: not by its last name, which prose such as "check .planroom/store.db" would match.
  for (const p of linked ?? []) {
    const lexical = normalize(p, places.cwd, places.home);
    if (secretPlaces(places).some((secret) => within(lexical, secret))) {
      return deny(
        readDenied(lexical, places) ??
          "Planroom's private files are read through the planroom CLI, never directly.",
      );
    }
    if (
      writesToo &&
      (within(lexical, places.dataRoot) ||
        pluginFolders(places).some((f) => within(lexical, f)))
    ) {
      return ask(writeDenied(lexical, places) ?? PLUGIN_WRITE);
    }
    for (const s of resolved(p)) {
      const reason = readDenied(s, places);
      if (reason !== null) return deny(reason);
      const write = writesToo ? writeDenied(s, places) : null;
      if (write !== null) return ask(write);
    }
  }
  for (const p of uploads) {
    for (const s of spell(p)) {
      const reason = uploadDenied(s, places);
      if (reason !== null) return deny(reason);
    }
  }
  const text = writtenText(tool, input);
  for (const p of writes) {
    for (const s of spell(p)) {
      const reason = writeDenied(s, places);
      if (reason !== null) return deny(reason);
      // What Bash may not export, no start-up or settings file sets for a later shell or session.
      if (
        text !== null &&
        setsEnvironment(s, places) &&
        namesGuardedVariable(text)
      ) {
        return deny(STARTUP_SET);
      }
    }
  }
  if (bash !== null) {
    let line = bash;
    let at = places;
    // A shell in a protected folder writes there through any program it runs, so the line must
    // leave first, with `cd DIR &&`; the rest is checked from where it goes.
    const sits = shellSitsDenied(places);
    if (sits !== null) {
      const leave = /^\s*(?:cd|pushd)\s+([^\s;&|]+)\s*(?:&&([\s\S]*))?$/.exec(
        bash,
      );
      if (leave === null) return deny(sits);
      const target = normalize(
        expand(unquote(leave[1] ?? ""), knownVariables(places)),
        places.cwd,
        places.home,
      );
      const away =
        cdDenied(target, places) ?? shellSitsDenied({ ...places, cwd: target });
      if (away !== null) return deny(away);
      line = leave[2] ?? "";
      at = { ...places, cwd: target };
      if (line.trim() === "") return null;
    }
    const forms = readings(line);
    for (const reading of forms) {
      const why = bashLineDenied(reading, resolved, at);
      if (why !== null) return deny(why);
    }
    for (const reading of forms) {
      const why = bashLineAsked(reading, resolved, at);
      if (why !== null) return { decision: "ask", reason: why };
    }
  }
  return null;
}

const STARTUP_WRITE =
  "This writes a shell's start-up file or a Claude Code settings file, which sets what later shells and sessions run with, so you decide whether it runs.";

/**
 * Why the person decides one reading of a Bash line nothing denies, or null: any write to a
 * start-up or settings file, whose content a staged copy or a `printf` doesn't show, and an
 * `env -S` whose line the guard can't split.
 */
function bashLineAsked(
  line: string,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
): string | null {
  const { bases, effects } = bashPaths(line, places);
  if (effects.question !== null) return effects.question;
  const startup = effects.writes.some((p) =>
    [...bases.map((b) => normalize(p, b, places.home)), ...resolved(p)].some(
      (s) => setsEnvironment(s, places),
    ),
  );
  return startup ? STARTUP_WRITE : null;
}

/** Why one reading of a Bash line is denied, or null. */
function bashLineDenied(
  line: string,
  resolved: (path: string) => readonly string[],
  places: GuardPlaces,
): string | null {
  const reason = bashDenied(line);
  if (reason !== null) return reason;
  // Each word from every directory the line may run it in; a glob by what it may expand to.
  const { words, args, bases, cds, effects } = bashPaths(line, places);
  if (effects.refusal !== null) return effects.refusal;
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
  /**
   * Whether a deleting find under this root (as written) selects a protected place: under each
   * folder it may name, when it holds a glob the shell expands.
   */
  const findReaches = (selects: Selection, root: string): string | null =>
    first([root], (s) => {
      const roots: [string, string][] = GLOB.test(s)
        ? globRoots(s, root, places).map((f) => [f, globWritten(root, s, f)])
        : [[s, root]];
      return roots.some(([at, as]) =>
        protectedSamples(at, places).some((sample) => {
          const rel = sample.path.slice(at.replace(/\/+$/, "").length);
          const written = as === "/" ? rel : `${as.replace(/\/+$/, "")}${rel}`;
          return selects(
            written,
            sample.dir,
            segmentsOf(rel).length,
            sample.empty,
          );
        }),
      )
        ? FIND_DELETE
        : null;
    });
  return (
    first(words, (s) =>
      mayName(s, engineDir(places), "within")
        ? "Planroom's engine cache runs with the person's authority: only the resolver writes it, and an agent's command never names it."
        : GLOB.test(s)
          ? globDenied(s, places)
          : readDenied(s, places),
    ) ??
    // security-r3-3: the plugin's folders are named only to run what's in them, such as the
    // resolver the planroom skill has the agent run; as any other word, a program may write them.
    first(args, (s) =>
      pluginFolders(places).some((p) => mayName(s, p, "within"))
        ? PLUGIN_WRITE
        : null,
    ) ??
    // security-r3-1: a host reports where the shell is, but a cd into a guarded place is refused
    // too, so a host that didn't still leaves no relative path there.
    first(cds, (s) => cdDenied(s, places)) ??
    // security-r2-1: what the Write tool may not write, no command or redirection writes either.
    first(effects.writes, (s) => writeTargetDenied(s, places)) ??
    // A guarded variable written into a start-up or settings file, such as JSON's `"PLANROOM_RAW":`.
    (namesGuardedVariable(line)
      ? first(effects.writes, (s) =>
          setsEnvironment(s, places) ? STARTUP_SET : null,
        )
      : null) ??
    first(effects.removes, (s) => removeDenied(s, places)) ??
    // A crafted tree unpacked or moved above a protected place could replace it.
    first(effects.unpacks, (s) => unpackDenied(s, places)) ??
    effects.arrivals
      .map((a) => first([a.dest], (s) => arrivalDenied(a, s, places)))
      .find((r) => r !== null) ??
    // security-r3-4: a find that deletes what it selects, by what it may select.
    effects.finds
      .flatMap((f) => f.roots.map((root) => findReaches(f.selects, root)))
      .find((r) => r !== null) ??
    // security-r2-2: a whole-tree read reaches whatever its folder holds.
    effects.trees
      .map((t) => first([t.word], (s) => treeDenied(s, t.hidden, places)))
      .find((r) => r !== null) ??
    null
  );
}
