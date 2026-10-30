/**
 * The lens switcher's entries, from the lenses `ask` lists as loaded (DR-122, DR-123): each is an
 * item whose one handle is `map --lens ID`. The canvas does not run that handle, since a lens is
 * which relationships you see and not where you are; it keeps the shown level and focus.
 */

import type { AskItem } from "@planroom/contract";
import { sanitize } from "@planroom/ui";

import type { PickerEntry } from "./shell/picker";

/** Whether an `ask` listing item is a lens: its one handle is a `map` that names a lens. */
export function isLensItem(item: AskItem): boolean {
  const [only, ...rest] = item.handles;
  return (
    only !== undefined &&
    rest.length === 0 &&
    only.command === "map" &&
    only.args.some((a) => a === "--lens" || a.startsWith("--lens="))
  );
}

/**
 * The picker's entries: the listing's lenses, the current one marked. Before the listing answers
 * (and on the Artifact, which never asks) the current lens alone, so the face still names it.
 */
export function lensEntries(
  items: readonly AskItem[],
  current: string | null,
): PickerEntry[] {
  const lenses = items.filter(isLensItem);
  if (lenses.length === 0) {
    return current === null
      ? []
      : [{ key: current, label: sanitize(current), current: true }];
  }
  return lenses.map((item) => ({
    key: item.id,
    label: sanitize(item.name),
    current: item.id === current,
  }));
}

/** The switcher's face: the current lens's name, else the lens ID, sanitized. */
export function lensLabel(
  items: readonly AskItem[],
  current: string | null,
): string {
  return (
    lensEntries(items, current).find((e) => e.current)?.label ??
    sanitize(current ?? "")
  );
}
