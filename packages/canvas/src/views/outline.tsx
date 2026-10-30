/**
 * The visually hidden outline of the map's rows, `role="tree"`, so a screen reader announces each
 * block with its kind, level and lifted edges (design-language.md, "Keyboard and screen readers").
 */

import { CANVAS_STRINGS, type LevelVocab } from "@planroom/contract";
import { sanitize, type BlockRow, type EdgeRow } from "@planroom/ui";

export function Outline({
  rows,
  edges,
  levels,
}: {
  /** The shown map's ladder, which names a block's level by its label. */
  levels: readonly LevelVocab[];
  rows: readonly BlockRow[];
  edges: readonly EdgeRow[];
}) {
  const names = new Map(rows.map((r) => [r.id, r.name]));
  const labels = new Map(levels.map((l) => [l.id, sanitize(l.label)]));
  return (
    <ul
      role="tree"
      aria-label={CANVAS_STRINGS.outline}
      className="visually-hidden"
    >
      {rows.map((row) => {
        const out = edges.filter((e) => e.from === row.id);
        return (
          <li key={row.id} role="treeitem" aria-level={1} aria-selected={false}>
            {[
              row.name,
              row.kindLabel,
              labels.get(row.level) ?? sanitize(row.level),
              ...row.countWords,
            ].join(", ")}
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
