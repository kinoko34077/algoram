import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react";
import { useEffect, useMemo, useState } from "react";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import {
  toFlowEdges,
  toFlowNodes,
  type FlowBlockNode,
} from "./graphAdapter";

interface GraphCanvasProps {
  graph: AlgoramGraph;
  selectedBlockId: string | null;
  onSelectBlock: (blockId: string | null) => void;
  onOpenGraph: (graphId: string, viaBlock: AlgoramBlock) => void;
}

function portTop(index: number, count: number): string {
  return `${((index + 1) / (count + 1)) * 100}%`;
}

function contractLabel(contract: unknown): string {
  if (typeof contract === "string") {
    return contract;
  }
  if (contract === undefined) {
    return "";
  }
  try {
    return JSON.stringify(contract);
  } catch {
    return String(contract);
  }
}

function BlockNode({ data, selected }: NodeProps<FlowBlockNode>) {
  const { block } = data;
  const ports = block.ports ?? [];
  const inputs = ports.filter((port) => port.direction === "in");
  const outputs = ports.filter((port) => port.direction === "out");

  return (
    <div className={selected ? "algoram-node selected" : "algoram-node"}>
      {inputs.map((port, index) => (
        <Handle
          key={port.id}
          id={port.id}
          type="target"
          position={Position.Left}
          className={`algoram-handle ${port.channel}`}
          style={{ top: portTop(index, inputs.length) }}
          title={`${port.channel} in ${contractLabel(port.contract)}`}
        />
      ))}

      <div className="node-title">{block.label}</div>
      <div className="node-meta">
        {block.internal_graph_ref ? <span>openable</span> : <span>leaf</span>}
        {block.source_anchor ? <span>source</span> : null}
      </div>

      {ports.length > 0 ? (
        <div className="port-summary">
          {ports.map((port) => (
            <span key={port.id}>
              {port.direction === "in" ? "←" : "→"} {port.id}
            </span>
          ))}
        </div>
      ) : null}

      {outputs.map((port, index) => (
        <Handle
          key={port.id}
          id={port.id}
          type="source"
          position={Position.Right}
          className={`algoram-handle ${port.channel}`}
          style={{ top: portTop(index, outputs.length) }}
          title={`${port.channel} out ${contractLabel(port.contract)}`}
        />
      ))}
    </div>
  );
}

const nodeTypes: NodeTypes = {
  algoramBlock: BlockNode,
};

function CanvasBody({
  graph,
  selectedBlockId,
  onSelectBlock,
  onOpenGraph,
}: GraphCanvasProps) {
  const [baseNodes, setBaseNodes] = useState<FlowBlockNode[]>([]);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const { fitView } = useReactFlow<FlowBlockNode>();

  const edges = useMemo(() => toFlowEdges(graph), [graph]);

  useEffect(() => {
    let cancelled = false;
    setLayoutError(null);

    void toFlowNodes(graph)
      .then((nextNodes) => {
        if (cancelled) {
          return;
        }
        setBaseNodes(nextNodes);
        requestAnimationFrame(() => {
          void fitView({ padding: 0.24, duration: 180 });
        });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLayoutError(
            error instanceof Error ? error.message : "Graph layout failed",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, [fitView, graph]);

  useEffect(() => {
    if (!selectedBlockId || !baseNodes.some((node) => node.id === selectedBlockId)) {
      return;
    }

    requestAnimationFrame(() => {
      void fitView({
        nodes: [{ id: selectedBlockId }],
        padding: 0.6,
        duration: 220,
      });
    });
  }, [baseNodes, fitView, selectedBlockId]);

  const nodes = useMemo(
    () =>
      baseNodes.map((node) => ({
        ...node,
        selected: node.id === selectedBlockId,
      })),
    [baseNodes, selectedBlockId],
  );

  return (
    <div className="graph-canvas">
      {layoutError ? <div className="canvas-error">{layoutError}</div> : null}
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={(_, node) => onSelectBlock(node.id)}
        onNodeDoubleClick={(_, node) => {
          const reference = node.data.block.internal_graph_ref;
          if (reference) {
            onOpenGraph(reference, node.data.block);
          }
        }}
        onPaneClick={() => onSelectBlock(null)}
        nodesConnectable={false}
        fitView
      >
        <Background gap={24} size={1} />
        <MiniMap pannable zoomable />
        <Controls />
      </ReactFlow>
    </div>
  );
}

export function GraphCanvas(props: GraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <CanvasBody {...props} />
    </ReactFlowProvider>
  );
}
