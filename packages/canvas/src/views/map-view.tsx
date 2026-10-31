/**
 * `MapView` (architecture.md, "The canvas"): the blocks at one level with their lifted edges and
 * counts, on React Flow, at the positions the store holds. Tap selects a block and dims all but
 * its neighbors; search dims what doesn't match. The map marks itself ready, for the e2e checks
 * and the `planroom:interactive` mark, once React Flow has measured every block.
 */

import {
  MarkerType,
  ReactFlow,
  NodeToolbar,
  Position,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type NodeChange,
  type XYPosition,
} from "@xyflow/react";
import {
  blocksOf,
  edgesOf,
  fill,
  matchBlocks,
  sanitize,
  yarnVar,
} from "@planroom/ui";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
} from "react";

import emptyMap from "../assets/empty-map.webp";
import { moveTargets, pendingMarks, relationKinds } from "../edits/ops";
import { BLOCK } from "../layout/graph";
import { ActionTray } from "../shell/action-tray";
import { useTopName } from "../shell/next-action";
import { CANVAS_STRINGS } from "@planroom/contract";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { BlockNodeComponent, type BlockNode } from "./block-node";
import { EmptyState } from "./empty-state";
import { drawsHere, emptyWords, inPane } from "./engine-empty";
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

/** A finger held this long on a block, in milliseconds, opens its tray (DR-68). */
const LONG_PRESS_MS = 500;
/** A finger or pointer that moves further than this, in px, is not holding still. */
const HOLD_SLOP_PX = 8;
/** A drag that ends within this many px of where it began changes nothing. */
const DRAG_SLOP_PX = 4;

/** One block being dragged: where it began, where it is, and the block under the pointer. */
interface Drag {
  id: string;
  start: XYPosition;
  at: XYPosition;
  over: string | null;
}

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
  const pending = useCanvas((s) => s.pending);
  const canEdit = useCanvas((s) => s.canEdit());
  const edit = useCanvas((s) => s.edit);
  const openTray = useCanvas((s) => s.openTray);
  const phone = useMedia(PHONE);
  const flow = useReactFlow();
  // Blocks move by hand only on the desktop; the phone pans and long presses instead.
  const drags = canEdit && !phone;
  const [ready, setReady] = useState(false);
  const wrapper = useRef<HTMLDivElement>(null);
  // Each block's measured size. A controlled flow draws a node it has no size for hidden until it
  // measures it again, so a block rebuilt on a selection would vanish for a frame and a second
  // press of a dive would land on the felt.
  const measured = useRef(new Map<string, { width: number; height: number }>());
  const lastPress = useRef<{ id: string; at: number } | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const dragRef = useRef<Drag | null>(null);
  // The chalk line just drawn: the block it ends on and a count that restarts the tray.
  const [relate, setRelate] = useState<{ to: string; n: number } | null>(null);
  // The phone's long press: the timer and where the finger went down, and the block whose
  // release click the press already used.
  const hold = useRef<{
    timer: ReturnType<typeof setTimeout>;
    x: number;
    y: number;
  } | null>(null);
  const held = useRef<string | null>(null);

  const rows = useMemo(() => (map ? blocksOf(map) : []), [map]);
  const kinds = useMemo(() => (map ? relationKinds(map) : []), [map]);
  const chalking = drags && kinds.length > 0;
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

  // What the workspace's unsent hand edits show now, before the engine's reply redraws the map.
  const marks = useMemo(() => pendingMarks(pending), [pending]);

  const nodes: BlockNode[] = useMemo(
    () =>
      rows
        .filter((r) => positions[r.id])
        .map((plain) => {
          const renamed = marks.names.get(plain.id);
          const row =
            renamed === undefined
              ? plain
              : { ...plain, name: sanitize(renamed) };
          return {
            id: row.id,
            type: "block" as const,
            position:
              drag !== null && drag.id === row.id
                ? drag.at
                : (marks.pinned.get(row.id) ?? positions[row.id]!),
            data: {
              row,
              dim:
                !matches.has(row.id) ||
                (near !== null && !near.has(row.id)) ||
                marks.hidden.has(row.id),
              pending: marks.marked.has(row.id),
              dropTarget: drag !== null && drag.over === row.id,
              chalk: chalking && row.id === selected,
            },
            selected: row.id === selected,
            measured: measured.current.get(row.id),
            draggable: drags,
            connectable: chalking,
            ariaLabel: `${row.name}, ${row.kindLabel}`,
          };
        }),
    [rows, positions, matches, near, selected, marks, drag, drags, chalking],
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

  // An unsent relation draws as a dashed chalk line between its two blocks.
  const chalked: Edge[] = useMemo(
    () =>
      marks.chalk
        .filter((c) => positions[c.from] && positions[c.to])
        .map((c) => ({
          id: `pending:${c.idem}`,
          source: c.from,
          target: c.to,
          className: "lifted pending-chalk",
          selectable: false,
          focusable: false,
        })),
    [marks, positions],
  );

  // The block a dragged block would move into: the one whose box holds the pointer, among the
  // blocks it may move into.
  const blockUnder = useCallback(
    (x: number, y: number, id: string): string | null => {
      if (!map) {
        return null;
      }
      const at = flow.screenToFlowPosition({ x, y });
      for (const target of moveTargets(map, id)) {
        const place = marks.pinned.get(target.id) ?? positions[target.id];
        if (!place) {
          continue;
        }
        const size = measured.current.get(target.id) ?? BLOCK;
        if (
          at.x >= place.x &&
          at.x <= place.x + size.width &&
          at.y >= place.y &&
          at.y <= place.y + size.height
        ) {
          return target.id;
        }
      }
      return null;
    },
    [map, flow, marks, positions],
  );

  const onNodeDragStart = useCallback(
    (_: unknown, node: { id: string; position: XYPosition }) => {
      dragRef.current = {
        id: node.id,
        start: { ...node.position },
        at: { ...node.position },
        over: null,
      };
      setDrag(dragRef.current);
    },
    [],
  );

  const onNodeDrag = useCallback(
    (event: MouseEvent | TouchEvent, node: { id: string }) => {
      const d = dragRef.current;
      if (d === null || d.id !== node.id) {
        return;
      }
      const point = "touches" in event ? event.touches[0] : event;
      const over = point
        ? blockUnder(point.clientX, point.clientY, d.id)
        : null;
      if (over !== d.over) {
        dragRef.current = { ...d, over };
        setDrag(dragRef.current);
      }
    },
    [blockUnder],
  );

  // A drop inside a block moves into it; a drop on the felt pins the block where it landed; a
  // drop back where it began, or within a few px, does nothing (DR-68).
  const onNodeDragStop = useCallback(
    (_: unknown, node: { id: string; position: XYPosition }) => {
      const d = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      if (d === null || d.id !== node.id) {
        return;
      }
      const moved = Math.hypot(
        node.position.x - d.start.x,
        node.position.y - d.start.y,
      );
      if (moved < DRAG_SLOP_PX) {
        return;
      }
      if (d.over !== null) {
        void edit({ op: "move", id: d.id, parent: d.over });
      } else {
        void edit({ op: "pin", id: d.id, ...node.position });
      }
    },
    [edit],
  );

  // A chalk line drawn onto another block: one relation kind sends at once, several ask which.
  const onConnect = useCallback(
    ({ source, target }: { source: string; target: string }) => {
      if (source === target) {
        return;
      }
      if (kinds.length === 1) {
        void edit({ op: "relate", from: source, to: target, kind: kinds[0]! });
      } else {
        setRelate({ to: target, n: performance.now() });
      }
    },
    [kinds, edit],
  );
  useEffect(() => setRelate(null), [selected]);

  const clearHold = () => {
    if (hold.current !== null) {
      clearTimeout(hold.current.timer);
      hold.current = null;
    }
  };
  useEffect(() => clearHold, []);

  // On the phone a finger held still on a block opens its tray (DR-68).
  const onPointerDown = (e: PointerEvent) => {
    held.current = null;
    if (!phone || !canEdit || e.pointerType === "mouse") {
      return;
    }
    const id = (e.target as Element)
      .closest(".react-flow__node")
      ?.getAttribute("data-id");
    if (!id) {
      return;
    }
    clearHold();
    hold.current = {
      x: e.clientX,
      y: e.clientY,
      timer: setTimeout(() => {
        hold.current = null;
        held.current = id;
        openTray(id);
      }, LONG_PRESS_MS),
    };
  };
  const onPointerMove = (e: PointerEvent) => {
    const h = hold.current;
    if (
      h !== null &&
      Math.hypot(e.clientX - h.x, e.clientY - h.y) > HOLD_SLOP_PX
    ) {
      clearHold();
    }
  };

  // Two presses on one block within the window go into it; one only selects. A tap and a click
  // both arrive here. The click that ends a long press has already had its use.
  const onNodeClick = useCallback(
    (_: unknown, node: { id: string }) => {
      if (held.current === node.id) {
        held.current = null;
        return;
      }
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
        } else if (
          c.type === "position" &&
          c.position &&
          dragRef.current?.id === c.id
        ) {
          dragRef.current = { ...dragRef.current, at: c.position };
          setDrag(dragRef.current);
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

  // One Tab from the selected block reaches its tray (design-language.md, "Keyboard and screen
  // readers"): the tray sits in the flow's overlay, far from the block in the tab order.
  const onKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key !== "Tab" || e.shiftKey || selected === null) {
        return;
      }
      const block = wrapper.current?.querySelector(
        `.react-flow__node[data-id="${CSS.escape(selected)}"]`,
      );
      const first = wrapper.current?.querySelector<HTMLElement>(
        ".action-tray button",
      );
      if (block !== null && block === document.activeElement && first) {
        e.preventDefault();
        first.focus();
      }
    },
    [selected],
  );

  return (
    <div
      ref={wrapper}
      className="map"
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={clearHold}
      onPointerCancel={clearHold}
      onContextMenu={phone ? (e) => e.preventDefault() : undefined}
      style={{ "--yarn": yarnVar(map?.yarn ?? null) } as CSSProperties}
      data-ready={ready || undefined}
      data-moment={moment}
      aria-label={CANVAS_STRINGS.map}
    >
      <ReactFlow<BlockNode, Edge>
        nodes={nodes}
        edges={chalked.length === 0 ? edges : [...edges, ...chalked]}
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
        nodesDraggable={drags}
        nodesConnectable={chalking}
        connectionLineStyle={{
          stroke: "var(--chalk)",
          strokeWidth: 2,
          strokeDasharray: "6 4",
        }}
        onConnect={onConnect}
        onNodeDragStart={onNodeDragStart}
        onNodeDrag={onNodeDrag}
        onNodeDragStop={onNodeDragStop}
        elementsSelectable
        zoomOnDoubleClick={false}
        onNodeClick={onNodeClick}
        onEdgeClick={
          lifted.some((e) => canOpenEvidence(e.from, e.to))
            ? onEdgeClick
            : undefined
        }
        onPaneClick={() => {
          setRelate(null);
          select(null);
          closeEvidence();
        }}
        proOptions={{ hideAttribution: true }}
      >
        <Revealer />
        {!phone && selected !== null && canEdit && (
          <NodeToolbar nodeId={selected} position={Position.Bottom} isVisible>
            <ActionTray
              key={relate?.n ?? 0}
              id={selected}
              startRelateTo={relate?.to}
            />
          </NodeToolbar>
        )}
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
  const canRun = useCanvas((s) => s.canRun);
  const back = useCanvas((s) => s.back);
  const top = useTopName();
  if (map === null) {
    return null;
  }
  const next = map.empty?.next ?? null;
  const words = map.empty == null ? null : emptyWords(map.empty);
  // The engine's next action is a button only when the canvas can run it and draw its answer;
  // otherwise a line names the pane that can.
  const action =
    next !== null && words !== null
      ? drawsHere(next) && canRun(next)
        ? { label: words.label, onPress: () => void run(next) }
        : null
      : {
          label: fill(CANVAS_STRINGS.back_to, { place: top }),
          onPress: () => void back(),
        };
  return (
    <div className="level-empty">
      <EmptyState
        art={emptyMap}
        concept="levels"
        line={words?.line}
        {...(action !== null
          ? { action }
          : next !== null
            ? { elsewhere: inPane(next) }
            : {})}
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
