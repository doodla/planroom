/**
 * The person key in `$.store` and the person's runs (architecture.md, "Actors"; DR-64, DR-72):
 * the key is made on the first session.start, its public half is registered once per workspace
 * with `planroom person init`, and every press, submit or typed `/room` runs the CLI with a fresh
 * `PLANROOM_PERSON_TOKEN` and no `PLANROOM_ACTOR`.
 */

import type { EngineInterface } from "claude-code";
import type { ErrorObject } from "@planroom/contract";

import {
  isPersonKey,
  KEY_STORE,
  lineFields,
  newPersonKey,
  signToken,
  type PersonKey,
} from "./actor";
import { parseReply } from "./engine";
import { clientEnv, runProcess } from "./live";

type $ = EngineInterface;

/** The person key, made and kept on first use; a stored value that doesn't check out is replaced. */
export async function personKey($: $): Promise<PersonKey> {
  const held = await $.store.get(KEY_STORE);
  if (isPersonKey(held)) {
    return held;
  }
  const key = newPersonKey();
  await $.store.set(KEY_STORE, key);
  return key;
}

/**
 * The environment for one person action: this session as the client and a token for exactly the
 * CLI line `argv` (after the binary), whose command's path takes `pathWords` words.
 */
export async function personEnv(
  $: $,
  argv: readonly string[],
  pathWords: number,
): Promise<Record<string, string>> {
  const key = await personKey($);
  const at = Math.floor((await $.clock.now()) / 1000);
  const token = signToken(key, { ...lineFields(argv, pathWords), at });
  return { ...(await clientEnv($)), PLANROOM_PERSON_TOKEN: token };
}

export type Registration =
  | { kind: "registered" }
  | { kind: "mismatch"; error: ErrorObject }
  | { kind: "not_now" };

/**
 * Register the key's public half with this workspace's store, once: `person init` reads it from
 * stdin. `rekey` replaces another registered key, which only the pane's press asks for (DR-75).
 * Until the engine has `person init`, or the workspace a store, this registers nothing and the
 * next start tries again.
 */
export async function registerKey(
  $: $,
  bin: string,
  workspace: string,
  rekey = false,
): Promise<Registration> {
  const key = await personKey($);
  if (!rekey && key.registered.includes(workspace)) {
    return { kind: "registered" };
  }
  const argv = ["person", "init", ...(rekey ? ["--rekey"] : []), "--json"];
  const run = await runProcess(
    $,
    [bin, ...argv],
    await personEnv($, argv, 2),
    30_000,
    `${key.public}\n`,
  );
  const reply = parseReply(run.stdout);
  if (reply?.ok === true) {
    const registered = [
      workspace,
      ...key.registered.filter((w) => w !== workspace),
    ];
    // A short list: the last workspaces this key was registered in.
    await $.store.set(KEY_STORE, {
      ...key,
      registered: registered.slice(0, 64),
    });
    return { kind: "registered" };
  }
  if (reply?.error?.code === "person_key_mismatch") {
    return { kind: "mismatch", error: reply.error };
  }
  return { kind: "not_now" };
}
