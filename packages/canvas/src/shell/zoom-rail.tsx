/**
 * The zoom rail (Levels), on the left edge at desktop widths: the shown map's levels in the
 * ladder's order, the one apart from the chain set apart, the shown one marked; a press asks
 * the map for that level. Before the first map arrives the rail draws no step.
 */

import { sanitize } from "@planroom/ui";

import { CANVAS_STRINGS } from "@planroom/contract";
import { atTop, topOf } from "../levels";
import { useCanvas } from "../use-canvas";

export function ZoomRail() {
  // The level and focus the engine drew, not the ones asked for.
  const level = useCanvas((s) => s.map?.level);
  const focus = useCanvas((s) => s.map?.focus ?? null);
  const levels = useCanvas((s) => s.map?.levels) ?? [];
  const top = topOf(levels)?.id ?? null;
  // The top is the first step of the chain: it is current at the top and at `--level <the top>`.
  const here = (id: string) =>
    id === top && level !== undefined ? atTop(level, focus, id) : id === level;
  const showLevel = useCanvas((s) => s.showLevel);
  return (
    <nav
      className="rail"
      data-concept="levels"
      aria-label={CANVAS_STRINGS.zoom_rail}
    >
      <ol>
        {levels.map((l) => {
          const label = sanitize(l.label);
          return (
            <li key={l.id} data-apart={l.apart || undefined}>
              <button
                type="button"
                className="rail-step"
                title={label}
                aria-current={here(l.id) ? "step" : undefined}
                onClick={() => void showLevel(l.id)}
              >
                <span className="rail-mark" aria-hidden="true" />
                <span className="rail-label">{label}</span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
