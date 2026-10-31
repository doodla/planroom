/**
 * One block: a chunky toy brick in its tone's color with a pale label plate, so the name always
 * reads at full contrast; its kind is printed on it, so color is never the only signal. A block
 * that holds others shows a lid edge (design-language.md, "Shape and depth").
 */

import {
  Handle,
  Position,
  useConnection,
  useUpdateNodeInternals,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import type { BlockRow } from "@planroom/ui";
import { toneVars } from "@planroom/ui";
import { CANVAS_STRINGS } from "@planroom/contract";
import { memo, useEffect, useRef, type CSSProperties } from "react";

export type BlockData = {
  row: BlockRow;
  /** Searched out or outside the selection's neighbors. */
  dim: boolean;
  /** An unsent hand edit touches this block (DR-68); it draws a dashed chalk border. */
  pending: boolean;
  /** A dragged block is over this one: its lid lifts and a drop here moves into it (DR-68). */
  dropTarget: boolean;
  /** This is the selected block and a relation can be drawn from it: it shows the chalk handle. */
  chalk: boolean;
};

export type BlockNode = Node<BlockData, "block">;

function BlockNodeView({ id, data, selected }: NodeProps<BlockNode>) {
  const { row, dim, pending, dropTarget, chalk } = data;
  // While a chalk line is being drawn, every block's target covers the block, so a drop anywhere
  // on it connects; otherwise the target stays out of the way of presses on the block.
  const drawing = useConnection((c) => c.inProgress);
  // React Flow measures a block's handles when the block resizes, not when a handle comes or goes.
  const measureHandles = useUpdateNodeInternals();
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    measureHandles(id);
  }, [id, chalk, drawing, measureHandles]);
  const tone = toneVars(row.tone);
  const style = { "--fill": tone.fill, "--edge": tone.border } as CSSProperties;
  return (
    <div
      className="block"
      style={style}
      data-tone={row.tone ?? "none"}
      data-holds={row.holds || undefined}
      data-ghost={row.ghost || undefined}
      data-dim={dim || undefined}
      data-pending={pending || undefined}
      data-drop-target={dropTarget || undefined}
      data-selected={selected || undefined}
    >
      {row.holds && <div className="block-lid" aria-hidden="true" />}
      <div className="block-plate">
        <div className="block-name">{row.name}</div>
        <div className="block-kind">{row.kindLabel}</div>
        {row.countWords.length > 0 && (
          <div className="block-counts">
            {row.countWords.map((word, i) => (
              <span key={i} className="block-count">
                {word}
              </span>
            ))}
          </div>
        )}
      </div>
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={drawing}
        isConnectableStart={false}
        className={drawing ? "block-handle drop-cover" : "block-handle"}
      />
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="block-handle"
      />
      {chalk && (
        <Handle
          id="chalk"
          type="source"
          position={Position.Right}
          isConnectable
          className="chalk-handle"
          data-concept="levels"
          aria-label={CANVAS_STRINGS.chalk}
        />
      )}
    </div>
  );
}

export const BlockNodeComponent = memo(BlockNodeView);
