/**
 * Whether this Claude Code can run the mod (architecture.md, "Wiring" step 1; DR-76, A-19).
 *
 * `missingMembers` and `hostLine` are pure: the first looks at the `$` it is handed, and the second
 * turns the session's version and the missing members into the one line a person sees, or `null`
 * when the host passes. A version that can't be read decides nothing; the member check decides
 * alone. `checkHost` is their wiring: it calls each member it can see on the real `$`.
 */

import type { EngineInterface, SessionVersion } from "claude-code";
import { MOD_STRINGS } from "@planroom/contract";
import { fill, sanitize, truncate } from "@planroom/ui";

type $ = EngineInterface;

/** The oldest Claude Code the mod runs on. Its home is `CLAUDE_CODE_VERSION` in crates/xtask/src/plugin.rs, which a unit test checks this against. */
export const MIN_HOST = "2.1.310";

// Every `$` member the mod calls, sorted; `fs.read` isn't listed because nothing calls it, so a host without it must not fail.
export const MEMBERS: readonly string[] = [
  "clock.after",
  "clock.now",
  "clock.sleep",
  "command.register",
  "env.get",
  "fs.stat",
  "plugin.root",
  "process.run",
  "process.spawn",
  "session.cwd",
  "session.id",
  "session.root",
  "session.version",
  "store.get",
  "store.set",
  "tool.register",
  "ui.invalidate",
  "ui.open",
  "ui.resolve",
  "ui.status",
  "ui.toast",
];

/** The members of `MEMBERS` that `api` lacks: not a function, or for `plugin.root` not a string. */
export function missingMembers(api: unknown): string[] {
  const missing: string[] = [];
  for (const member of MEMBERS) {
    const [noun = "", name = ""] = member.split(".");
    let value: unknown;
    try {
      const group = (api as Record<string, unknown> | null | undefined)?.[noun];
      value = (group as Record<string, unknown> | null | undefined)?.[name];
    } catch {
      value = undefined;
    }
    const present =
      member === "plugin.root"
        ? typeof value === "string"
        : typeof value === "function";
    if (!present) missing.push(member);
  }
  return missing;
}

/** `major.minor.patch` from the leading digits of a version, or `null` when it doesn't start with them. */
function core(version: string | undefined): [number, number, number] | null {
  const m = /^(\d+)\.(\d+)\.(\d+)/.exec(version ?? "");
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

function below(a: readonly number[], b: readonly number[]): boolean {
  for (let i = 0; i < 3; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x !== y) return x < y;
  }
  return false;
}

/** The line to show when the host can't run the mod, or `null` when it can. */
export function hostLine(
  version: SessionVersion | null,
  missing: readonly string[],
): string | null {
  const read = version === null ? null : core(version.base ?? version.version);
  const minimum = core(MIN_HOST) ?? [0, 0, 0];
  if (version !== null && read !== null && below(read, minimum)) {
    return sanitize(
      fill(MOD_STRINGS.host_too_old, {
        version: version.version,
        minimum: MIN_HOST,
      }),
    );
  }
  if (missing.length > 0) {
    const shown =
      version !== null && read !== null
        ? version.version
        : MOD_STRINGS.host_unknown_version;
    return sanitize(fill(MOD_STRINGS.host_changed, { version: shown }));
  }
  return null;
}

/**
 * The `$` members a call can't see: each has an effect (it registers, runs, writes, opens or
 * shows something), or, for `ui.resolve`, needs a render event, so the check takes it as present.
 * Their first use is the probe: `session.start` registers `/room` and the tools and opens the pane
 * (or `session.attach` opens it), and a TypeError there (`isMissing`) fails the host as the check
 * does (`changedLine`); any other failure is a refusal, which leaves the mod on (`refusedLine`).
 * The rest fail at their call, which the mod's callers survive: `ui.status` through
 * `showHostLine`, `process.run` as a failed run, a render hook as one the host skips.
 */
export const UNPROBED: ReadonlySet<string> = new Set([
  "command.register",
  "process.run",
  "process.spawn",
  "store.set",
  "tool.register",
  "ui.open",
  "ui.resolve",
  "ui.status",
  "ui.toast",
]);

/**
 * What `$` holds for each member the mod calls, as `missingMembers` reads it, and the session's
 * version. The host's loader refuses a bundle that hands `$` to a function, or reads a `$` member
 * as a value (`typeof` included), so the only way to see a member is to call it where it is
 * spelled, or to read `$.plugin.root`. Every member whose call leaves nothing behind is called
 * here; a call to an absent member throws a TypeError, and that member is missing. The members in
 * `UNPROBED` are taken as present.
 */
async function seenOn($: $): Promise<{
  seen: Record<string, Record<string, unknown>>;
  version: SessionVersion | null;
}> {
  const gone = new Set<string>();
  // A TypeError is a member that isn't there; any other throw is a member that answered badly.
  const called = async (member: string, call: () => unknown): Promise<void> => {
    try {
      await call();
    } catch (err) {
      if (isMissing(err)) gone.add(member);
    }
  };
  let version: SessionVersion | null = null;
  await called("session.version", async () => {
    version = await $.session.version();
  });
  await called("clock.now", () => $.clock.now());
  // Not awaited: a sleep of 0 only has to be asked for, and a stopped clock never ends it.
  await called("clock.sleep", () => {
    void Promise.resolve($.clock.sleep(0)).catch(() => undefined);
  });
  let timer: { cancel: () => void } | undefined;
  await called("clock.after", () => {
    timer = $.clock.after(0, () => undefined);
  });
  try {
    timer?.cancel();
  } catch {
    // A timer that can't be stopped runs a function that does nothing.
  }
  await called("session.cwd", () => $.session.cwd());
  await called("session.id", () => $.session.id());
  await called("session.root", () => $.session.root());
  await called("env.get", () => $.env.get("HOME"));
  await called("store.get", () => $.store.get(HOST_CHECK_KEY));
  await called("fs.stat", () => $.fs.stat("/"));
  await called("ui.invalidate", () => $.ui.invalidate("ui.render"));
  let root: unknown;
  try {
    root = $.plugin.root;
  } catch {
    root = undefined;
  }
  const seen: Record<string, Record<string, unknown>> = {};
  for (const member of MEMBERS) {
    const [noun = "", name = ""] = member.split(".");
    const group = seen[noun] ?? {};
    group[name] =
      member === "plugin.root"
        ? root
        : gone.has(member)
          ? undefined
          : () => undefined;
    seen[noun] = group;
  }
  return { seen, version };
}

/**
 * Whether a `$` call failed because the host lacks the member: a TypeError, thrown at the call or
 * the reason its promise rejects with, as calling a member that isn't there throws one. Its name
 * counts too, since an error made in another realm isn't this one's TypeError. Any other failure is
 * a refusal, never a missing member: a hook's deny, which rejects the caller's promise, or the
 * host's own, such as a command named like a built-in (DR-129).
 */
export function isMissing(err: unknown): boolean {
  return (
    err instanceof TypeError ||
    (typeof err === "object" &&
      err !== null &&
      (err as { name?: unknown }).name === "TypeError")
  );
}

/** The most of a refusal's reason the refused line shows. */
const REASON_CHARS = 200;

/**
 * The one line a refused registration or pane gets: `host_refused`, naming `what` was refused and
 * the reason the refusal gave, its first line only, without the host's `<plugin>: $.<call>: `
 * prefix, cut to `REASON_CHARS` and sanitized. The reason is the host's or a hook's own words; the
 * mod adds nothing of its own to it, so no credential of the mod's can reach the line.
 */
export function refusedLine(what: string, err: unknown): string {
  const said =
    err instanceof Error
      ? err.message
      : typeof err === "string"
        ? err
        : String(err);
  const first = (said.split("\n")[0] ?? "")
    .replace(/^\S+: \$\.[\w.]+: /, "")
    .trim();
  return sanitize(
    fill(MOD_STRINGS.host_refused, {
      what,
      reason: truncate(first, REASON_CHARS),
    }),
  );
}

/** A store key nothing writes, which the check reads to see `store.get`. */
const HOST_CHECK_KEY = "planroom.host-check";

/**
 * The line a host that can't run the mod gets, or `null` when it can, with the `$` members it lacks
 * and the version the session reported (`null` when it couldn't say).
 */
export async function checkHost($: $): Promise<{
  line: string | null;
  missing: string[];
  version: SessionVersion | null;
}> {
  const { seen, version } = await seenOn($);
  const missing = missingMembers(seen);
  return { line: hostLine(version, missing), missing, version };
}

/**
 * The line for a host that passed the check but whose `member`, one of `UNPROBED`, failed at its
 * first use: `host_changed`, as the check gives for a missing member, with the version it read.
 */
export function changedLine(
  version: SessionVersion | null,
  member: string,
): string {
  return (
    hostLine(version, [member]) ??
    sanitize(
      fill(MOD_STRINGS.host_changed, {
        version: MOD_STRINGS.host_unknown_version,
      }),
    )
  );
}

/** Shows the host's line on the status line, when the host has one to show it on. */
export function showHostLine($: $, line: string): void {
  try {
    $.ui.status(line);
  } catch {
    // `ui.status` is in UNPROBED: a host without it shows nothing, and the session still starts.
  }
}
