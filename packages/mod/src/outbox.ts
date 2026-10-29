/**
 * The outbox (architecture.md, "Idempotency and retries"; DR-8): every write the mod starts is kept
 * in the `outbox` atom from before its process starts until any reply, so a reload mid-write sends
 * it again with the same `--idem`, argv, stdin and cause, as the same actor and client, and the
 * store's request hash makes it one event. An entry holds the names of the variables its run set,
 * never their values, so no token or key is ever in `$.state`. Pure: register.tsx wires it.
 */

import type { OutboxEntry } from "@planroom/contract";

import { agentEnv } from "./tools";

/** The value after `--idem` in `argv`, either form. */
export function idemOf(argv: readonly string[]): string | null {
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] ?? "";
    if (a === "--idem") return argv[i + 1] ?? null;
    if (a.startsWith("--idem=")) return a.slice("--idem=".length);
  }
  return null;
}

/** The entry for one write: its argv after the binary, its env's names, its stdin and cause. */
export function entryFor(
  argv: readonly string[],
  env: Readonly<Record<string, string>>,
  stdin: string | undefined,
  session: string,
): OutboxEntry | null {
  const idem = idemOf(argv);
  if (idem === null) return null;
  const entry: OutboxEntry = {
    idem,
    argv: [...argv],
    env_names: Object.keys(env).sort(),
    session,
  };
  if (stdin !== undefined) entry.stdin = stdin;
  const cause = env["PLANROOM_CAUSE"];
  if (cause !== undefined) entry.cause = cause;
  return entry;
}

/** The outbox with `entry` added, replacing one with the same `idem`. */
export function add(
  outbox: readonly OutboxEntry[],
  entry: OutboxEntry,
): OutboxEntry[] {
  return [...outbox.filter((e) => e.idem !== entry.idem), entry];
}

/** The outbox without the entry for `idem`. */
export function clear(
  outbox: readonly OutboxEntry[],
  idem: string,
): OutboxEntry[] {
  return outbox.filter((e) => e.idem !== idem);
}

/**
 * Whether `entry` is another session's: only the session that ran it resends it, since a resend
 * from another would be another actor and client, so another request hash.
 */
export function stale(entry: OutboxEntry, sessionId: string): boolean {
  return entry.session !== sessionId;
}

/** Whether `entry` was a person's write, which a resend signs a fresh token for. */
export function byPerson(entry: OutboxEntry): boolean {
  return entry.env_names.includes("PLANROOM_PERSON_TOKEN");
}

/**
 * What a resend runs: the same argv, stdin and `PLANROOM_CAUSE`, with `PLANROOM_ACTOR` and
 * `PLANROOM_CLIENT` recomputed from the session ID, so they equal the first run's. A person's
 * entry gets its fresh token from the caller, for the same argv.
 */
export function resend(
  entry: OutboxEntry,
  sessionId: string,
): { argv: string[]; env: Record<string, string>; stdin?: string } {
  const env: Record<string, string> = {};
  if (entry.env_names.includes("PLANROOM_ACTOR")) {
    Object.assign(env, agentEnv(sessionId, null));
  }
  if (entry.env_names.includes("PLANROOM_CLIENT")) {
    env["PLANROOM_CLIENT"] = `mod:${sessionId}`;
  }
  if (entry.cause !== undefined && entry.cause !== null) {
    env["PLANROOM_CAUSE"] = entry.cause;
  }
  const out: { argv: string[]; env: Record<string, string>; stdin?: string } = {
    argv: [...entry.argv],
    env,
  };
  if (entry.stdin !== undefined && entry.stdin !== null)
    out.stdin = entry.stdin;
  return out;
}
