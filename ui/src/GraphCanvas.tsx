import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useReactFlow,
  type Connection,
  type Edge,
  type NodeChange,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import {
  toFlowEdges,
  toFlowNodes,
  type FlowBlockNode,
} from "./graphAdapter";
import {
  addDraftLink,
  clearDraftLinks,
  makeDraftLink,
  resetNodePositions,
  setNodePosition,
  validateDraftLink,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";

interface GraphCanvasProps {
  graph: AlgoramGraph;
  selectedBlockId: string | null;
  presentation: GraphPresentationState;
  onPresentationChange: (
    update: (current: GraphPresentationState) => GraphPresentationState,
  ) => void;
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
      <span className={`port-dot ${channel}`} aria-hidden="true" />
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
          title={`${port.channel} input ${contractLabel(port.contract)}`}
          aria-label={`${block.label} ${port.id} input`}
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
          title={`${port.channel} output ${contractLabel(port.contract)}`}
          aria-label={`${block.label} ${port.id} output`}
        />
      ))}
    </div>
  );
}

const nodeTypes: NodeTypes = {
  algoramBlock: BlockNode,
};

function connectionToDraft(connection: Connection | Edge): DraftLink | null {
  if (
    !connection.source ||
    !connection.target ||
    !connection.sourceHandle ||
    !connection.targetHandle
  ) {
    return null;
  }

  return makeDraftLink(
    connection.source,
    connection.sourceHandle,
    connection.target,
    connection.targetHandle,
  );
}

function draftToEdge(link: DraftLink): Edge {
  return {
    id: link.id,
    source: link.sourceBlockId,
    target: link.targetBlockId,
    sourceHandle: link.sourcePortId,
    targetHandle: link.targetPortId,
    animated: true,
    className: "draft-edge",
    label: "draft",
  };
}

function CanvasBody({
  graph,
  selectedBlockId,
  presentation,
  onPresentationChange,
  onSelectBlock,
  onOpenGraph,
}: GraphCanvasProps) {
  const [baseNodes, setBaseNodes, onNodesChange] =
    useNodesState<FlowBlockNode>([]);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const [interactionStatus, setInteractionStatus] = useState<string | null>(
    null,
  );
  const { fitView } = useReactFlow<FlowBlockNode>();

  const edges = useMemo(
    () => [
      ...toFlowEdges(graph),
      ...presentation.draftLinks.map(draftToEdge),
    ],
    [graph, presentation.draftLinks],
  );

  const loadLayout = useCallback(
    async (useSavedPositions: boolean) => {
      setLayoutError(null);
      try {
        const nextNodes = await toFlowNodes(graph);
        const positioned = useSavedPositions
          ? nextNodes.map((node) => ({
              ...node,
              position: presentation.positions[node.id] ?? node.position,
            }))
          : nextNodes;

        setBaseNodes(positioned);
        requestAnimationFrame(() => {
          void fitView({ padding: 0.2, duration: 160 });
        });
      } catch (error: unknown) {
        setLayoutError(
          error instanceof Error ? error.message : "Graph layout failed",
        );
      }
    },
    [fitView, graph, presentation.positions, setBaseNodes],
  );

  useEffect(() => {
    void loadLayout(true);
    setInteractionStatus(null);
  }, [graph.id]);

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
        padding: 0.55,
        duration: 180,
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

  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowBlockNode>[]) => {
      onNodesChange(changes);

      const positioned = changes.filter(
        (
          change,
        ): change is Extract<
          NodeChange<FlowBlockNode>,
          { type: "position" }
        > => change.type === "position" && change.position !== undefined,
      );

      if (positioned.length === 0) {
        return;
      }

      onPresentationChange((current) => {
        let next = current;
        for (const change of positioned) {
          if (change.position) {
            next = setNodePosition(next, change.id, change.position);
          }
        }
        return next;
      });
    },
    [onNodesChange, onPresentationChange],
  );

  const isValidConnection = useCallback(
    (connection: Connection | Edge) => {
      const link = connectionToDraft(connection);
      return (
        link !== null &&
        validateDraftLink(graph, presentation, link) === null
      );
    },
    [graph, presentation],
  );

  const connectDraft = useCallback(
    (connection: Connection) => {
      const link = connectionToDraft(connection);
      if (!link) {
        setInteractionStatus("Choose an output and input port.");
        return;
      }

      const error = validateDraftLink(graph, presentation, link);
      if (error) {
        setInteractionStatus(error);
        return;
      }

      onPresentationChange((current) => addDraftLink(graph, current, link));
      setInteractionStatus("Draft link added.");
    },
    [graph, onPresentationChange, presentation],
  );

  const resetLayout = useCallback(() => {
    onPresentationChange(resetNodePositions);
    void loadLayout(false);
    setInteractionStatus("Layout reset to the automatic arrangement.");
  }, [loadLayout, onPresentationChange]);

  const clearDrafts = useCallback(() => {
    onPresentationChange(clearDraftLinks);
    setInteractionStatus("Draft links cleared.");
  }, [onPresentationChange]);

  return (
    <div className="graph-canvas">
      <div className="canvas-toolbar" aria-label="Node workspace controls">
        <div className="workspace-mode">
          <strong>Node workspace</strong>
          <span>presentation only</span>
        </div>

        <div className="canvas-toolbar-actions">
          <button
            type="button"
            className="secondary-action"
            onClick={resetLayout}
            disabled={Object.keys(presentation.positions).length === 0}
          >
            Reset layout
          </button>
          <button
            type="button"
            className="secondary-action"
            onClick={clearDrafts}
            disabled={presentation.draftLinks.length === 0}
          >
            Clear drafts
          </button>
        </div>

        <p className="canvas-help">
          Arrow keys move a selected node. Click or drag handles to connect.
          Graph JSON stays unchanged.
        </p>

        <div className="canvas-status" aria-live="polite">
          <span>{presentation.draftLinks.length} drafts</span>
          {interactionStatus ? <span>{interactionStatus}</span> : null}
          {layoutError ? (
            <span className="error-text" role="alert">
              {layoutError}
            </span>
          ) : null}
        </div>
      </div>

      <div className="canvas-surface">
        <ReactFlow
          aria-label="Algoram graph canvas"
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={handleNodesChange}
          onConnect={connectDraft}
          isValidConnection={isValidConnection}
          onNodeClick={(_, node) => onSelectBlock(node.id)}
          onSelectionChange={({ nodes: selectedNodes }) =>
            onSelectBlock(selectedNodes.at(-1)?.id ?? null)
          }
          onNodeDoubleClick={(_, node) => {
            const reference = node.data.block.internal_graph_ref;
            if (reference) {
              onOpenGraph(reference, node.data.block);
            }
          }}
          onPaneClick={() => onSelectBlock(null)}
          nodesConnectable
          nodesDraggable
          nodesFocusable
          edgesFocusable
          connectOnClick
          disableKeyboardA11y={false}
          deleteKeyCode={null}
          fitView
          colorMode="dark"
          connectionLineStyle={{
            stroke: "var(--color-accent)",
            strokeWidth: 2,
          }}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={22}
            size={1}
            color="var(--color-canvas-dot)"
          />
          <MiniMap
            pannable
            zoomable
            nodeColor={(node) =>
              node.selected
                ? "var(--color-accent)"
                : "var(--color-node-muted)"
            }
            maskColor="rgb(6 8 11 / 72%)"
          />
          <Controls />
        </ReactFlow>
      </div>
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
