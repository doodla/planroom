/**
 * `MapView` (architecture.md, "The canvas"): the blocks at one level with their lifted edges and
 * counts, on React Flow, at the positions the store holds. Tap selects a block and dims all but
 * its neighbors; search dims what doesn't match. The map marks itself ready, for the e2e checks
 * and the `planroom:interactive` mark, once React Flow has measured every block.
 */

import {
  MarkerType,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type NodeChange,
} from "@xyflow/react";
import { LEVELS, blocksOf, edgesOf, matchBlocks } from "@planroom/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { BLOCK } from "../layout/graph";
import { STRINGS } from "../strings";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { BlockNodeComponent, type BlockNode } from "./block-node";
import { Outline } from "./outline";

const NODE_TYPES = { block: BlockNodeComponent };

/** Centers a block the search revealed. */
function Revealer() {
  const flow = useReactFlow();
  const revealed = useCanvas((s) => s.revealed);
  const positions = useCanvas((s) => s.positions);
  useEffect(() => {
    const at = revealed && positions[revealed.id];
    if (at) {
      void flow.setCenter(at.x + BLOCK.width / 2, at.y + BLOCK.height / 2, {
        zoom: Math.max(flow.getZoom(), 0.8),
      });
    }
  }, [revealed, positions, flow]);
  return null;
}

type Moment = "dive" | "rise" | undefined;

function MapFlow({ moment }: { moment: Moment }) {
  const map = useCanvas((s) => s.map);
  const positions = useCanvas((s) => s.positions);
  const selected = useCanvas((s) => s.selected);
  const query = useCanvas((s) => s.query);
  const select = useCanvas((s) => s.select);
  const phone = useMedia(PHONE);
  const [ready, setReady] = useState(false);
  const measured = useRef(new Set<string>());

  const rows = useMemo(() => (map ? blocksOf(map) : []), [map]);
  const lifted = useMemo(() => (map ? edgesOf(map) : []), [map]);
  const matches = useMemo(() => matchBlocks(rows, query), [rows, query]);
  const near = useMemo(() => {
    if (selected === null) {
      return null;
    }
    const ids = new Set([selected]);
    for (const e of lifted) {
      if (e.from === selected) ids.add(e.to);
      if (e.to === selected) ids.add(e.from);
    }
    return ids;
  }, [selected, lifted]);

  const nodes: BlockNode[] = useMemo(
    () =>
      rows
        .filter((r) => positions[r.id])
        .map((row) => ({
          id: row.id,
          type: "block" as const,
          position: positions[row.id]!,
          data: {
            row,
            dim: !matches.has(row.id) || (near !== null && !near.has(row.id)),
          },
          selected: row.id === selected,
          draggable: false,
          connectable: false,
          ariaLabel: `${row.name}, ${row.kindLabel}`,
        })),
    [rows, positions, matches, near, selected],
  );

  const edges: Edge[] = useMemo(
    () =>
      lifted.map((e) => {
        const dim = near !== null && e.from !== selected && e.to !== selected;
        return {
          id: e.id,
          source: e.from,
          target: e.to,
          label: e.count > 1 ? String(e.count) : undefined,
          ariaLabel: `${e.kindLabel} ×${e.count}`,
          className: dim
            ? "lifted dim"
            : near !== null
              ? "lifted near"
              : "lifted",
          markerEnd: {
            type: MarkerType.ArrowClosed,
            width: 18,
            height: 18,
            color: "var(--muted)",
          },
          focusable: false,
          selectable: false,
          labelBgPadding: [6, 3] as [number, number],
          labelBgBorderRadius: 6,
        };
      }),
    [lifted, near, selected],
  );

  // A controlled flow learns each block's size from its dimension changes.
  const onNodesChange = useCallback(
    (changes: NodeChange<BlockNode>[]) => {
      for (const c of changes) {
        if (c.type === "dimensions" && c.dimensions) {
          measured.current.add(c.id);
        }
      }
      if (
        !ready &&
        nodes.length > 0 &&
        nodes.every((n) => measured.current.has(n.id))
      ) {
        setReady(true);
        if (performance.getEntriesByName("planroom:interactive").length === 0) {
          performance.mark("planroom:interactive");
        }
      }
    },
    [ready, nodes],
  );

  return (
    <div
      className="map"
      data-ready={ready || undefined}
      data-moment={moment}
      aria-label={STRINGS.map}
    >
      <ReactFlow<BlockNode, Edge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        onNodesChange={onNodesChange}
        fitView
        fitViewOptions={{
          padding: 0.12,
          maxZoom: 1,
          minZoom: phone ? 0.5 : 0.25,
        }}
        minZoom={0.2}
        maxZoom={2}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable
        zoomOnDoubleClick={false}
        onNodeClick={(_, node) => select(node.id)}
        onPaneClick={() => select(null)}
        proOptions={{ hideAttribution: true }}
      >
        <Revealer />
      </ReactFlow>
      <Outline rows={rows} edges={lifted} />
    </div>
  );
}

/** The map with its own React Flow provider, remounted with a level zoom for each new level. */
export function MapCanvas() {
  const level = useCanvas((s) => s.map?.level ?? null);
  const [shown, setShown] = useState(level);
  const [moment, setMoment] = useState<Moment>(undefined);
  if (level !== shown) {
    // The lid lifts going down a level and drops back going up (design-language.md, "Motion").
    const rank = (l: typeof level) => (l === null ? -1 : LEVELS.indexOf(l));
    setMoment(rank(level) > rank(shown) ? "dive" : "rise");
    setShown(level);
  }
  return (
    <ReactFlowProvider key={level ?? ""}>
      <MapFlow moment={moment} />
    </ReactFlowProvider>
  );
}
