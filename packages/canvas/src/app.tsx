/** The canvas: the shell around the map, or a full-page empty state with its one next action. */

import emptyMap from "./assets/empty-map.webp";
import { Shell } from "./shell/shell";
import { STRINGS } from "./strings";
import { useCanvas } from "./use-canvas";
import { EmptyState } from "./views/empty-state";

export function App() {
  const phase = useCanvas((s) => s.phase);
  const problem = useCanvas((s) => s.problem);
  // The top level's empty state fills the page; a level below it keeps the shell, so its
  // breadcrumb and rail still lead back up (ux-3).
  const empty = useCanvas((s) =>
    s.level === null ? (s.map?.empty ?? null) : null,
  );
  const start = useCanvas((s) => s.start);
  const run = useCanvas((s) => s.run);

  switch (phase) {
    case "starting":
      return (
        <div className="starting" aria-busy="true">
          {STRINGS.starting}
        </div>
      );
    case "needs_login":
      return (
        <main className="page">
          <EmptyState art={emptyMap} line={STRINGS.needsLogin} />
        </main>
      );
    case "unreachable":
      return (
        <main className="page">
          <EmptyState
            art={emptyMap}
            line={problem ?? STRINGS.unreachable}
            action={{ label: STRINGS.retry, onPress: () => void start() }}
          />
        </main>
      );
    case "ready":
      if (empty !== null) {
        return (
          <main className="page">
            <EmptyState
              art={emptyMap}
              action={{
                label: empty.next.label,
                onPress: () => void run(empty.next),
              }}
            />
          </main>
        );
      }
      return <Shell />;
  }
}
