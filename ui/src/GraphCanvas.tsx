import {
  addEdge,
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Connection,
  type Edge,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

function PortLabel({
  direction,
  channel,
  id,
}: {
  direction: "in" | "out";
  channel: "flow" | "data";
  id: string;
}) {
  return (
    <span className={`node-port-label ${direction}`}>
      <span className={`port-dot ${channel}`} />
      <span>{id}</span>
    </span>
  );
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

      <div className="node-titlebar">
        <span className="node-grip" aria-hidden="true">
          ⠿
        </span>
        <div className="node-title">{block.label}</div>
        {block.internal_graph_ref ? (
          <span className="node-badge">graph</span>
        ) : (
          <span className="node-badge muted">block</span>
        )}
      </div>

      <div className="node-subtitle">
        {block.implementation_ref ?? block.definition_ref ?? block.id}
      </div>

      {ports.length > 0 ? (
        <div className="node-port-grid">
          <div className="node-port-column inputs">
            {inputs.map((port) => (
              <PortLabel
                key={port.id}
                direction="in"
                channel={port.channel}
                id={port.id}
              />
            ))}
          </div>
          <div className="node-port-column outputs">
            {outputs.map((port) => (
              <PortLabel
                key={port.id}
                direction="out"
                channel={port.channel}
                id={port.id}
              />
            ))}
          </div>
        </div>
      ) : (
        <div className="node-empty-ports">no exposed ports</div>
      )}

      <div className="node-meta">
        {block.source_anchor ? <span>source</span> : null}
        {block.diagnostics?.length ? (
          <span>{block.diagnostics.length} diag</span>
        ) : null}
      </div>

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
  const [baseNodes, setBaseNodes, onNodesChange] =
    useNodesState<FlowBlockNode>([]);
  const [edges, setEdges] = useEdgesState<Edge>([]);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const [draftCount, setDraftCount] = useState(0);
  const draftSequence = useRef(0);
  const { fitView } = useReactFlow<FlowBlockNode>();

  const restoreCanonicalEdges = useCallback(() => {
    setEdges(toFlowEdges(graph));
    draftSequence.current = 0;
    setDraftCount(0);
  }, [graph, setEdges]);

  const autoLayout = useCallback(async () => {
    setLayoutError(null);
    try {
      const nextNodes = await toFlowNodes(graph);
      setBaseNodes(nextNodes);
      requestAnimationFrame(() => {
        void fitView({ padding: 0.2, duration: 180 });
      });
    } catch (error: unknown) {
      setLayoutError(
        error instanceof Error ? error.message : "Graph layout failed",
      );
    }
  }, [fitView, graph, setBaseNodes]);

  useEffect(() => {
    let cancelled = false;
    setLayoutError(null);
    restoreCanonicalEdges();

    void toFlowNodes(graph)
      .then((nextNodes) => {
        if (cancelled) {
          return;
        }
        setBaseNodes(nextNodes);
        requestAnimationFrame(() => {
          void fitView({ padding: 0.2, duration: 180 });
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
  }, [fitView, graph, restoreCanonicalEdges, setBaseNodes]);

  useEffect(() => {
    if (
      !selectedBlockId ||
      !baseNodes.some((node) => node.id === selectedBlockId)
    ) {
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

  const connectDraft = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) {
        return;
      }

      const draftId = `draft:${graph.id}:${draftSequence.current}`;
      draftSequence.current += 1;

      setEdges((current) =>
        addEdge(
          {
            ...connection,
            id: draftId,
            animated: true,
            className: "draft-edge",
            label: "draft",
          },
          current,
        ),
      );
      setDraftCount((count) => count + 1);
    },
    [graph.id, setEdges],
  );

  return (
    <div className="graph-canvas comfy-canvas">
      {layoutError ? <div className="canvas-error">{layoutError}</div> : null}

      <div className="canvas-toolbar" aria-label="Node workspace controls">
        <div className="workspace-mode">
          <strong>Node workspace</strong>
          <span>presentation-only editing</span>
        </div>
        <div className="canvas-toolbar-actions">
          <button type="button" onClick={() => void autoLayout()}>
            Auto layout
          </button>
          <button
            type="button"
            className="secondary"
            onClick={restoreCanonicalEdges}
            disabled={draftCount === 0}
          >
            Clear draft links
          </button>
        </div>
        <small>
          {draftCount} draft {draftCount === 1 ? "link" : "links"} · drag nodes
          freely · Graph JSON unchanged
        </small>
      </div>

      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onConnect={connectDraft}
        onNodeClick={(_, node) => onSelectBlock(node.id)}
        onNodeDoubleClick={(_, node) => {
          const reference = node.data.block.internal_graph_ref;
          if (reference) {
            onOpenGraph(reference, node.data.block);
          }
        }}
        onPaneClick={() => onSelectBlock(null)}
        nodesConnectable
        nodesDraggable
        deleteKeyCode={null}
        fitView
        colorMode="dark"
        connectionLineStyle={{ stroke: "#a978ff", strokeWidth: 2 }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={22}
          size={1.2}
          color="#343b46"
        />
        <MiniMap
          pannable
          zoomable
          nodeColor={(node) => (node.selected ? "#a978ff" : "#46505e")}
          maskColor="rgb(10 12 16 / 72%)"
        />
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
