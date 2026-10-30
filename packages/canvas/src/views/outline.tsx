/**
 * The visually hidden outline of the map's rows, `role="tree"`, so a screen reader announces each
 * block with its kind, level and lifted edges (design-language.md, "Keyboard and screen readers").
 */

import type { BlockRow, EdgeRow } from "@planroom/ui";

import { STRINGS } from "../strings";

export function Outline({
  rows,
  edges,
}: {
  rows: readonly BlockRow[];
  edges: readonly EdgeRow[];
}) {
  const names = new Map(rows.map((r) => [r.id, r.name]));
  return (
    <ul role="tree" aria-label={STRINGS.outline} className="visually-hidden">
      {rows.map((row) => {
        const out = edges.filter((e) => e.from === row.id);
        return (
          <li key={row.id} role="treeitem" aria-level={1} aria-selected={false}>
            {[row.name, row.kindLabel, row.level, ...row.countWords].join(", ")}
            {out.length > 0 && (
              <ul role="group">
                {out.map((e) => (
                  <li
                    key={e.id}
                    role="treeitem"
                    aria-level={2}
                    aria-selected={false}
                  >
                    {`${e.kindLabel} → ${names.get(e.to) ?? e.to}, ${e.count}`}
                  </li>
                ))}
              </ul>
            )}
          </li>
        );
      })}
    </ul>
  );
}
