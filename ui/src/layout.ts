import ELK from "elkjs/lib/elk-api.js";
import ElkWorker from "elkjs/lib/elk-worker.min.js?worker";
import type { ElkNode } from "elkjs/lib/elk-api.js";
import type { AlgoramGraph } from "./algoram";
import { getBlockGeometry } from "./blockDisplay";

export interface NodePosition {
  x: number;
  y: number;
}

const elk = new ELK({
  workerFactory: () => new ElkWorker(),
});

export async function layoutGraph(
  graph: AlgoramGraph,
): Promise<Map<string, NodePosition>> {
  const input: ElkNode = {
    id: graph.id,
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.spacing.nodeNode": "48",
      "elk.layered.spacing.nodeNodeBetweenLayers": "96",
      "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
    },
    children: graph.blocks.map((block) => ({
      id: block.id,
      ...getBlockGeometry(block),
    })),
    edges: graph.connections.map((connection) => ({
      id: connection.id,
      sources: [connection.source.block_id],
      targets: [connection.target.block_id],
    })),
  };

  const result = await elk.layout(input);
  const positions = new Map<string, NodePosition>();

  for (const child of result.children ?? []) {
    positions.set(child.id, {
      x: child.x ?? 0,
      y: child.y ?? 0,
    });
  }

  return positions;
}
