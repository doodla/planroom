/**
 * The zoom rail (Levels), on the left edge at desktop widths: the five levels, the current one
 * marked; a press asks the map for that level.
 */

import { LEVELS } from "@planroom/ui";

import { STRINGS } from "../strings";
import { useCanvas } from "../use-canvas";

export function ZoomRail() {
  // The level asked for, even while its view waits on a request.
  const level = useCanvas((s) => s.level ?? s.map?.level);
  const showLevel = useCanvas((s) => s.showLevel);
  return (
    <nav className="rail" data-concept="levels" aria-label={STRINGS.zoomRail}>
      <ol>
        {LEVELS.map((l) => (
          <li key={l}>
            <button
              type="button"
              className="rail-step"
              aria-current={l === level ? "step" : undefined}
              onClick={() => void showLevel(l)}
            >
              <span className="rail-mark" aria-hidden="true" />
              <span className="rail-label">{l}</span>
            </button>
          </li>
        ))}
      </ol>
    </nav>
  );
}
