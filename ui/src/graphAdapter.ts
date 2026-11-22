import {
  MarkerType,
  type Edge,
  type Node,
} from "@xyflow/react";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import { layoutGraph } from "./layout";
import { projectBlockDisplay, type BlockDisplayProjection } from "./blockDisplay";

export type BlockNodeData = Record<string, unknown> & {
  block: AlgoramBlock;
  display: BlockDisplayProjection;
};

export type FlowBlockNode = Node<BlockNodeData, "algoramBlock">;

export async function toFlowNodes(
  graph: AlgoramGraph,
): Promise<FlowBlockNode[]> {
  const positions = await layoutGraph(graph);

  return graph.blocks.map((block) => {
    const display = projectBlockDisplay(block);
    return {
      id: block.id,
      type: "algoramBlock",
      position: positions.get(block.id) ?? { x: 0, y: 0 },
      style: { width: display.geometry.width, height: display.geometry.height },
      ariaLabel: display.fullLabel,
      data: { block, display },
    };
  });
}

export function toFlowEdges(graph: AlgoramGraph): Edge[] {
  return graph.connections.map((connection) => ({
    id: connection.id,
    source: connection.source.block_id,
    target: connection.target.block_id,
    sourceHandle: connection.source.port_id,
    targetHandle: connection.target.port_id,
    markerEnd: { type: MarkerType.ArrowClosed },
    label: connection.id,
    className: "canonical-edge",
    focusable: true,
    selectable: true,
  }));
}
