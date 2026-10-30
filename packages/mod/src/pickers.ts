/**
 * The pickers the mod deals (architecture.md, "Pane"; DR-77): which command asks for one, which
 * key picks an entry and which presses close it. Pure: register.tsx keeps the open picker and runs
 * the picks, and layout.ts draws it and gives each entry its fixed key.
 */

import {
  CATALOGUE,
  type CatalogueLeaf,
  type Handle,
  type Picker,
} from "@planroom/contract";

import { pickerKeys } from "./layout";

/**
 * The argv of the `focus` that deals the picker `name`, before `--json` and `--session`. The
 * project is the last one the person picked, if they picked one; the stage picker counts that
 * plan's items.
 */
export function pickerArgv(name: string, project: string | null): string[] {
  return [
    "focus",
    "--picker",
    name,
    ...(project === null || project === "" ? [] : ["--project", project]),
  ];
}

/** The entry `key` picks: its handle, or null for a key no entry holds or an entry that is a label. */
export function pick(
  picker: Picker,
  key: string,
  leaves: readonly CatalogueLeaf[] = CATALOGUE,
): Handle | null {
  const at = pickerKeys(picker, leaves).indexOf(key);
  return picker.entries[at]?.handle ?? null;
}

/**
 * Whether pressing `key` closes the open picker: any action key other than the picker's own, which
 * deals it again.
 */
export function closes(key: string, own: string): boolean {
  return key !== own;
}
