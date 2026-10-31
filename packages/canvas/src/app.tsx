/** The canvas: the shell around the map, or a full-page empty state with its one next action. */

import { sanitize } from "@planroom/ui";

import emptyMap from "./assets/empty-map.webp";
import { atTop, topOf } from "./levels";
import { Shell } from "./shell/shell";
import { CANVAS_STRINGS } from "@planroom/contract";
import { useCanvas } from "./use-canvas";
import { EmptyState } from "./views/empty-state";
import {
  drawsHere,
  emptyWords,
  fillsThePage,
  inPane,
} from "./views/engine-empty";

export function App() {
  const phase = useCanvas((s) => s.phase);
  const problem = useCanvas((s) => s.problem);
  const hint = useCanvas((s) => s.hint);
  // The top level's empty state fills the page; a level below it keeps the shell, so its
  // breadcrumb and rail still lead back up (ux-3), and so does a top that holds a plan, so its
  // project shelf is on screen.
  const empty = useCanvas((s) => {
    const map = s.map;
    if (map === null || map.empty == null || !fillsThePage(map.empty)) {
      return null;
    }
    const top = topOf(map.levels ?? [])?.id ?? null;
    return top !== null && s.lens === null && atTop(map.level, map.focus, top)
      ? map.empty
      : null;
  });
  const start = useCanvas((s) => s.start);
  const run = useCanvas((s) => s.run);
  const canRun = useCanvas((s) => s.canRun);

  switch (phase) {
    case "starting":
      return (
        <div className="starting" aria-busy="true">
          {CANVAS_STRINGS.starting}
        </div>
      );
    case "needs_login":
      return (
        <main className="page">
          <EmptyState art={emptyMap} line={CANVAS_STRINGS.needs_login} />
        </main>
      );
    case "unreachable":
      return (
        <main className="page">
          <EmptyState
            art={emptyMap}
            concept="levels"
            line={sanitize(problem ?? CANVAS_STRINGS.unreachable)}
            {...(hint === null ? {} : { hint })}
            action={{
              label: CANVAS_STRINGS.retry,
              onPress: () => void start(),
            }}
          />
        </main>
      );
    case "ready":
      if (empty !== null) {
        // The engine's line says why the page is empty and what to do; the button is its short
        // action, which says little alone ("Check again").
        const words = emptyWords(empty);
        const runs = drawsHere(empty.next) && canRun(empty.next);
        return (
          <main className="page">
            <EmptyState
              art={emptyMap}
              concept="levels"
              line={words.line}
              {...(runs
                ? {
                    action: {
                      label: words.label,
                      onPress: () => void run(empty.next),
                    },
                  }
                : { elsewhere: inPane(empty.next) })}
            />
          </main>
        );
      }
      return <Shell />;
  }
}
