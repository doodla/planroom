/**
 * A handle as the request `POST /api/v1/{command}` takes (contracts.md, "HTTP API and live
 * stream"): the route reads the command's request JSON, not its CLI arguments, so the local
 * transport turns a handle's `args` into that JSON. `--name value` sets `name`, with dashes as
 * underscores; a `--name` with no value after it sets `name` to true; a positional argument fills
 * the command's next positional field. Values stay strings, as the handle spells them.
 */

import type { Handle } from "@planroom/contract";

/**
 * Each command's positional fields, in order, as its request struct names them. `decide ID
 * CHOICE` (contracts.md) has no request struct until M2; its fields are named as its synopsis
 * names them.
 */
const POSITIONALS: Readonly<Record<string, readonly string[]>> = {
  scan: ["dir"],
  decide: ["id", "choice"],
};

export function requestOf(handle: Handle): Record<string, unknown> {
  const request: Record<string, unknown> = {};
  const positionals = [...(POSITIONALS[handle.command] ?? [])];
  const args = handle.args;
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (arg.startsWith("--")) {
      const eq = arg.indexOf("=");
      const flag = eq < 0 ? arg.slice(2) : arg.slice(2, eq);
      const name = flag.replaceAll("-", "_");
      const next = args[i + 1];
      if (eq >= 0) {
        request[name] = arg.slice(eq + 1);
      } else if (next !== undefined && !next.startsWith("--")) {
        request[name] = next;
        i++;
      } else {
        request[name] = true;
      }
      continue;
    }
    const field = positionals.shift();
    if (field === undefined) {
      throw new Error(
        `${handle.command} takes no positional argument here: ${arg}`,
      );
    }
    request[field] = arg;
  }
  return request;
}
