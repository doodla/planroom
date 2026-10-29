/** The generated command catalogue, as the mod's tools, `/room` and the band read it. */

import {
  CATALOGUE,
  ROOM_COMMANDS,
  type CatalogueLeaf,
  type RoomCommand,
  type ToolName,
} from "@planroom/contract";

export type { ToolName };

/** The leaf a command name names, such as `map` or `handoff merge`. */
export function leaf(name: string): CatalogueLeaf | undefined {
  return CATALOGUE.find((l) => l.name === name);
}

/** The leaf a command line after the binary runs: the longest name its leading words spell. */
export function leafOfArgv(argv: readonly string[]): CatalogueLeaf | undefined {
  let best: CatalogueLeaf | undefined;
  for (const l of CATALOGUE) {
    const path = l.name.split(" ");
    if (
      path.every((p, i) => argv[i] === p) &&
      (best === undefined || path.length > best.name.split(" ").length)
    ) {
      best = l;
    }
  }
  return best;
}

/** The leaves a tool serves, in catalogue order. */
export function leavesFor(tool: ToolName): CatalogueLeaf[] {
  return CATALOGUE.filter((l) => l.tool === tool);
}

/** The leaves reachable as `/room ...`. */
export function roomLeaves(): CatalogueLeaf[] {
  return CATALOGUE.filter((l) => l.room);
}

/** The `/room` commands, with their synopses, subcommands and flags, in catalogue order. */
export function roomCommands(): readonly RoomCommand[] {
  return ROOM_COMMANDS;
}

/**
 * Whether a stray digit at the empty prompt may run this command from the band: never an
 * irreversible, hand-off or spending leaf, nor one the catalogue doesn't know (DR-54, DR-66).
 * Anything else shows on the band as a label that opens its card in the pane.
 */
export function bandMayRun(
  command: string,
  leaves: readonly CatalogueLeaf[] = CATALOGUE,
): boolean {
  const l = leaves.find((c) => c.name === command);
  return (
    l !== undefined &&
    l.undo !== "not_undoable" &&
    l.class !== "handoff" &&
    !l.spends
  );
}
