/**
 * The zoom rail (Levels), on the left edge at desktop widths: the shown map's levels in the
 * ladder's order, the one apart from the chain set apart, the current one marked; a press asks
 * the map for that level. Before the first map arrives the rail draws no step.
 */

import { sanitize } from "@planroom/ui";

import { STRINGS } from "../strings";
import { useCanvas } from "../use-canvas";

export function ZoomRail() {
  // The level asked for, even while its view waits on a request.
  const level = useCanvas((s) => s.level ?? s.map?.level);
  const levels = useCanvas((s) => s.map?.levels) ?? [];
  const showLevel = useCanvas((s) => s.showLevel);
  return (
    <nav className="rail" data-concept="levels" aria-label={STRINGS.zoomRail}>
      <ol>
        {levels.map((l) => {
          const label = sanitize(l.label);
          return (
            <li key={l.id} data-apart={l.apart || undefined}>
              <button
                type="button"
                className="rail-step"
                title={label}
                aria-current={l.id === level ? "step" : undefined}
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
