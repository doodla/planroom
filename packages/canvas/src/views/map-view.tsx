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
import { blocksOf, edgesOf, fill, matchBlocks, yarnVar } from "@planroom/ui";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";

import emptyMap from "../assets/empty-map.webp";
import { BLOCK } from "../layout/graph";
import { useTopName } from "../shell/next-action";
import { CANVAS_STRINGS } from "@planroom/contract";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { BlockNodeComponent, type BlockNode } from "./block-node";
import { EmptyState } from "./empty-state";
import { emptyWords } from "./engine-empty";
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

/** Two presses on one block this close together, in milliseconds, are a dive. */
const DIVE_WINDOW_MS = 300;

function MapFlow({ moment }: { moment: Moment }) {
  const map = useCanvas((s) => s.map);
  const positions = useCanvas((s) => s.positions);
  const selected = useCanvas((s) => s.selected);
  const query = useCanvas((s) => s.query);
  const select = useCanvas((s) => s.select);
  const dive = useCanvas((s) => s.dive);
  const canOpenEvidence = useCanvas((s) => s.canOpenEvidence);
  const openEvidence = useCanvas((s) => s.openEvidence);
  const closeEvidence = useCanvas((s) => s.closeEvidence);
  const phone = useMedia(PHONE);
  const [ready, setReady] = useState(false);
  // Each block's measured size. A controlled flow draws a node it has no size for hidden until it
  // measures it again, so a block rebuilt on a selection would vanish for a frame and a second
  // press of a dive would land on the felt.
  const measured = useRef(new Map<string, { width: number; height: number }>());
  const lastPress = useRef<{ id: string; at: number } | null>(null);

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
          measured: measured.current.get(row.id),
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
        const openable = canOpenEvidence(e.from, e.to);
        return {
          id: e.id,
          source: e.from,
          target: e.to,
          label: e.count > 1 ? String(e.count) : undefined,
          ariaLabel: `${e.kindLabel} ×${e.count}`,
          className:
            (dim ? "lifted dim" : near !== null ? "lifted near" : "lifted") +
            (openable ? " openable" : ""),
          interactionWidth: 24,
          markerEnd: {
            type: MarkerType.ArrowClosed,
            width: 18,
            height: 18,
            color: "var(--yarn)",
          },
          focusable: false,
          selectable: false,
          labelBgPadding: [6, 3] as [number, number],
          labelBgBorderRadius: 6,
        };
      }),
    [lifted, near, selected, canOpenEvidence],
  );

  // Two presses on one block within the window go into it; one only selects. A tap and a click
  // both arrive here.
  const onNodeClick = useCallback(
    (_: unknown, node: { id: string }) => {
      const now = performance.now();
      const last = lastPress.current;
      if (
        last !== null &&
        last.id === node.id &&
        now - last.at <= DIVE_WINDOW_MS
      ) {
        lastPress.current = null;
        void dive(node.id);
        return;
      }
      lastPress.current = { id: node.id, at: now };
      select(node.id);
    },
    [dive, select],
  );

  // A press on an edge asks the engine why its two blocks are related (architecture.md, A14).
  const onEdgeClick = useCallback(
    (_: unknown, edge: Edge) => {
      void openEvidence(edge.source, edge.target);
    },
    [openEvidence],
  );

  // A controlled flow learns each block's size from its dimension changes.
  const onNodesChange = useCallback(
    (changes: NodeChange<BlockNode>[]) => {
      for (const c of changes) {
        if (c.type === "dimensions" && c.dimensions) {
          measured.current.set(c.id, c.dimensions);
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
      style={{ "--yarn": yarnVar(map?.yarn ?? null) } as CSSProperties}
      data-ready={ready || undefined}
      data-moment={moment}
      aria-label={CANVAS_STRINGS.map}
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
        onNodeClick={onNodeClick}
        onEdgeClick={
          lifted.some((e) => canOpenEvidence(e.from, e.to))
            ? onEdgeClick
            : undefined
        }
        onPaneClick={() => {
          select(null);
          closeEvidence();
        }}
        proOptions={{ hideAttribution: true }}
      >
        <Revealer />
      </ReactFlow>
      <Outline levels={map?.levels ?? []} rows={rows} edges={lifted} />
    </div>
  );
}

/**
 * A level with no blocks is never bare felt (design-language.md, "Empty states"; ux-3): the
 * engine's empty state when it sends one, its line and its next action as the button; otherwise
 * a line of the canvas's own, and the way back up to the top.
 */
function LevelEmpty() {
  const map = useCanvas((s) => s.map);
  const run = useCanvas((s) => s.run);
  const back = useCanvas((s) => s.back);
  const top = useTopName();
  if (map === null) {
    return null;
  }
  const next = map.empty?.next ?? null;
  const words = map.empty == null ? null : emptyWords(map.empty);
  return (
    <div className="level-empty">
      <EmptyState
        art={emptyMap}
        concept="levels"
        line={words?.line}
        action={
          next !== null && words !== null
            ? { label: words.label, onPress: () => void run(next) }
            : {
                label: fill(CANVAS_STRINGS.back_to, { place: top }),
                onPress: () => void back(),
              }
        }
      />
    </div>
  );
}

/** The map with its own React Flow provider, remounted with a level zoom for each new level. */
export function MapCanvas() {
  const blank = useCanvas((s) => s.map !== null && s.map.nodes.length === 0);
  if (blank) {
    return <LevelEmpty />;
  }
  return <LevelMap />;
}

/** Where the map is, as the lids and the rail say it: what a new view is compared with. */
interface Seen {
  readonly key: string;
  readonly rank: number;
  readonly depth: number;
}

function LevelMap() {
  const level = useCanvas((s) => s.map?.level ?? null);
  const focus = useCanvas((s) => s.map?.focus ?? null);
  const lens = useCanvas((s) => s.map?.lens ?? null);
  const depth = useCanvas((s) => s.trail.length);
  const levels = useCanvas((s) => s.map?.levels) ?? [];
  const key = `${level ?? ""}|${focus ?? ""}|${lens ?? ""}`;
  const rank = level === null ? -1 : levels.findIndex((e) => e.id === level);
  const [shown, setShown] = useState<Seen>({ key, rank, depth });
  const [moment, setMoment] = useState<Moment>(undefined);
  if (key !== shown.key || depth !== shown.depth) {
    // The lid lifts going down and drops back going up (design-language.md, "Motion"): down is a
    // block pushed on the trail or a later level of the shown map, up a pop or an earlier one. A
    // new lens alone moves neither.
    const down =
      depth !== shown.depth ? depth > shown.depth : rank > shown.rank;
    const up = depth !== shown.depth ? depth < shown.depth : rank < shown.rank;
    setMoment(down ? "dive" : up ? "rise" : undefined);
    setShown({ key, rank, depth });
  }
  // Every new view gets its own flow, so `fitView` fits it, not only a new level.
  return (
    <ReactFlowProvider key={key}>
      <MapFlow moment={moment} />
    </ReactFlowProvider>
  );
}
