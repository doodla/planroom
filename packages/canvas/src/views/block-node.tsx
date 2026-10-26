/**
 * One block: a chunky toy brick in its tone's color with a pale label plate, so the name always
 * reads at full contrast; its kind is printed on it, so color is never the only signal. A block
 * that holds others shows a lid edge (design-language.md, "Shape and depth").
 */

import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { BlockRow } from "@planroom/ui";
import { toneVars } from "@planroom/ui";
import { memo, type CSSProperties } from "react";

export type BlockData = {
  row: BlockRow;
  /** Searched out or outside the selection's neighbors. */
  dim: boolean;
};

export type BlockNode = Node<BlockData, "block">;

function BlockNodeView({ data, selected }: NodeProps<BlockNode>) {
  const { row, dim } = data;
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
      data-selected={selected || undefined}
    >
      {row.holds && <div className="block-lid" aria-hidden="true" />}
      <div className="block-plate">
        <div className="block-name">{row.name}</div>
        <div className="block-kind">{row.kindLabel}</div>
        {row.counts.length > 0 && (
          <div className="block-counts">
            {row.counts.map((c) => `${c.count} ${c.level}`).join(" · ")}
          </div>
        )}
      </div>
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="block-handle"
      />
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="block-handle"
      />
    </div>
  );
}

export const BlockNodeComponent = memo(BlockNodeView);
