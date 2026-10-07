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
  type XYPosition,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import { BlockPalette } from "./BlockPalette";
import type { ReusableBlockTemplate } from "./blockAuthoring";
import { markEditorPerformance } from "./perfMarks";
import {
  toFlowEdges,
  toFlowNodes,
  type FlowBlockNode,
} from "./graphAdapter";
import {
  clearDraftLinks,
  makeDraftLink,
  resetNodePositions,
  setNodePosition,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";

interface GraphFocusRequest {
  blockId: string;
  revision: number;
}

interface GraphCanvasProps {
  graph: AlgoramGraph;
  editable: boolean;
  blockTemplates: ReusableBlockTemplate[];
  authoringStatus: string | null;
  selectedBlockId: string | null;
  selectedConnectionId: string | null;
  focusRequest: GraphFocusRequest | null;
  presentation: GraphPresentationState;
  onPresentationChange: (
    update: (current: GraphPresentationState) => GraphPresentationState,
  ) => void;
  onAddBlock: (
    template: ReusableBlockTemplate,
    position: XYPosition,
  ) => void;
  validateCanonicalConnection: (link: DraftLink) => string | null;
  onAddConnection: (link: DraftLink) => string | null;
  onSelectBlock: (blockId: string | null) => void;
  onSelectConnection: (connectionId: string | null) => void;
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

const nodeA11yDescription =
  "Press Enter or Space to select a node. Use the arrow keys to move a selected node. Press Escape to clear selection.";

const ariaLabelConfig = {
  "node.a11yDescription.default": nodeA11yDescription,
  "node.a11yDescription.keyboardDisabled": nodeA11yDescription,
  "edge.a11yDescription.default":
    "Canonical Connection between Blocks. Select it to inspect or remove it when the Graph is editable.",
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
    focusable: false,
    selectable: false,
  };
}

function graphStructureKey(graph: AlgoramGraph): string {
  return JSON.stringify({
    blocks: graph.blocks.map((block) => [
      block.id,
      (block.ports ?? []).map((port) => [
        port.id,
        port.direction,
        port.channel,
      ]),
    ]),
    connections: graph.connections.map((connection) => [
      connection.id,
      connection.source.block_id,
      connection.source.port_id,
      connection.target.block_id,
      connection.target.port_id,
    ]),
  });
}

function CanvasBody({
  graph,
  editable,
  blockTemplates,
  authoringStatus,
  selectedBlockId,
  selectedConnectionId,
  focusRequest,
  presentation,
  onPresentationChange,
  onAddBlock,
  validateCanonicalConnection,
  onAddConnection,
  onSelectBlock,
  onSelectConnection,
  onOpenGraph,
}: GraphCanvasProps) {
  const [baseNodes, setBaseNodes, onNodesChange] =
    useNodesState<FlowBlockNode>([]);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const [interactionStatus, setInteractionStatus] = useState<string | null>(
    null,
  );
  const [paletteOpen, setPaletteOpen] = useState(true);
  const pointerDragActive = useRef(false);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [loadedGraphId, setLoadedGraphId] = useState<string | null>(null);
  const [loadedStructureKey, setLoadedStructureKey] = useState<string | null>(
    null,
  );
  const { fitView, getNode, screenToFlowPosition } =
    useReactFlow<FlowBlockNode>();

  const structureKey = useMemo(() => graphStructureKey(graph), [graph]);

  const edges = useMemo(
    () => [
      ...toFlowEdges(graph).map((edge) => ({
        ...edge,
        selected: edge.id === selectedConnectionId,
      })),
      ...presentation.draftLinks.map(draftToEdge),
    ],
    [graph, presentation.draftLinks, selectedConnectionId],
  );

  const loadLayout = useCallback(
    async (
      useSavedPositions: boolean,
      fitViewport: boolean,
      preserveCurrentPositions: boolean,
    ) => {
      setLayoutError(null);
      try {
        const nextNodes = await toFlowNodes(graph);
        setBaseNodes((currentNodes) => {
          const currentPositions = new Map(
            currentNodes.map((node) => [node.id, node.position]),
          );

          return nextNodes.map((node) => ({
            ...node,
            position:
              (useSavedPositions
                ? presentation.positions[node.id]
                : undefined) ??
              (preserveCurrentPositions
                ? currentPositions.get(node.id)
                : undefined) ??
              node.position,
          }));
        });
        setLoadedGraphId(graph.id);
        setLoadedStructureKey(structureKey);

        if (fitViewport) {
          requestAnimationFrame(() => {
            void fitView({ padding: 0.2, duration: 160 });
          });
        }
      } catch (error: unknown) {
        setLayoutError(
          error instanceof Error ? error.message : "Graph layout failed",
        );
      }
    },
    [
      fitView,
      graph,
      presentation.positions,
      setBaseNodes,
      structureKey,
    ],
  );

  useEffect(() => {
    void loadLayout(true, true, false);
    setInteractionStatus(null);
  }, [graph.id]);

  useEffect(() => {
    if (
      loadedGraphId !== graph.id ||
      loadedStructureKey === null ||
      loadedStructureKey === structureKey
    ) {
      return;
    }

    void loadLayout(true, false, true);
  }, [graph.id, loadedGraphId, loadedStructureKey, structureKey]);

  useEffect(() => {
    if (
      !focusRequest ||
      loadedGraphId !== graph.id ||
      !getNode(focusRequest.blockId)
    ) {
      return;
    }

    requestAnimationFrame(() => {
      void fitView({
        nodes: [{ id: focusRequest.blockId }],
        padding: 0.55,
        duration: 180,
      });
    });
  }, [fitView, focusRequest, getNode, graph.id, loadedGraphId]);

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

      const selected = changes.find(
        (
          change,
        ): change is Extract<
          NodeChange<FlowBlockNode>,
          { type: "select" }
        > => change.type === "select" && change.selected,
      );
      if (selected) {
        onSelectBlock(selected.id);
      } else if (
        selectedBlockId &&
        changes.some(
          (change) =>
            change.type === "select" &&
            !change.selected &&
            change.id === selectedBlockId,
        )
      ) {
        onSelectBlock(null);
      }

      const settledPositions = changes.filter(
        (
          change,
        ): change is Extract<
          NodeChange<FlowBlockNode>,
          { type: "position" }
        > =>
          change.type === "position" &&
          change.position !== undefined &&
          change.dragging !== true &&
          !pointerDragActive.current,
      );

      if (settledPositions.length === 0) {
        return;
      }

      onPresentationChange((current) => {
        let next = current;
        for (const change of settledPositions) {
          if (change.position) {
            next = setNodePosition(next, change.id, change.position);
          }
        }
        return next;
      });
    },
    [
      onNodesChange,
      onPresentationChange,
      onSelectBlock,
      selectedBlockId,
    ],
  );

  const commitNodePosition = useCallback(
    (node: FlowBlockNode) => {
      onPresentationChange((current) =>
        setNodePosition(current, node.id, node.position),
      );
    },
    [onPresentationChange],
  );

  const isValidConnection = useCallback(
    (connection: Connection | Edge) => {
      if (!editable) {
        return false;
      }
      const link = connectionToDraft(connection);
      return link !== null && validateCanonicalConnection(link) === null;
    },
    [editable, validateCanonicalConnection],
  );

  const connectCanonical = useCallback(
    (connection: Connection) => {
      if (!editable) {
        return;
      }

      const link = connectionToDraft(connection);
      if (!link) {
        setInteractionStatus("Choose an output and input port.");
        return;
      }

      const error = onAddConnection(link);
      setInteractionStatus(error ?? "Canonical Connection created.");
    },
    [editable, onAddConnection],
  );

  const resetLayout = useCallback(() => {
    onPresentationChange(resetNodePositions);
    void loadLayout(false, true, false);
    setInteractionStatus("Layout reset to the automatic arrangement.");
  }, [loadLayout, onPresentationChange]);

  const clearDrafts = useCallback(() => {
    onPresentationChange(clearDraftLinks);
    setInteractionStatus("Draft links cleared.");
  }, [onPresentationChange]);

  const addFromPalette = useCallback(
    (template: ReusableBlockTemplate) => {
      const surface = surfaceRef.current;
      const fallback = { x: 40, y: 40 };
      let position = fallback;

      if (surface) {
        const bounds = surface.getBoundingClientRect();
        const center = screenToFlowPosition({
          x: bounds.left + bounds.width / 2,
          y: bounds.top + bounds.height / 2,
        });
        position = {
          x: center.x - 122,
          y: center.y - 48,
        };
      }

      onAddBlock(template, position);
      setInteractionStatus(`Added ${template.label}.`);
    },
    [onAddBlock, screenToFlowPosition],
  );

  return (
    <div className="graph-canvas">
      <div className="canvas-toolbar" aria-label="Node workspace controls">
        <div className="workspace-mode">
          <strong>Node workspace</strong>
          <span>
            {editable
              ? "canonical authoring + presentation"
              : "read-only Graph + presentation"}
          </span>
        </div>

        <div className="canvas-toolbar-actions">
          {editable ? (
            <button
              type="button"
              className="tertiary-action"
              aria-controls="block-palette"
              aria-expanded={paletteOpen}
              aria-pressed={paletteOpen}
              onClick={() => setPaletteOpen((open) => !open)}
            >
              Library
            </button>
          ) : null}
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
          {editable
            ? "Library Add and handle connections change the canonical Graph. Drag/layout remain presentation-only."
            : "Canonical connection handles are disabled. Draft planning remains in the Inspector."}
        </p>

        <div className="canvas-status" aria-live="polite">
          <span>{presentation.draftLinks.length} drafts</span>
          {authoringStatus ? <span>{authoringStatus}</span> : null}
          {interactionStatus ? <span>{interactionStatus}</span> : null}
          {layoutError ? (
            <span className="error-text" role="alert">
              {layoutError}
            </span>
          ) : null}
        </div>
      </div>

      <div
        className={
          editable && paletteOpen
            ? "canvas-work-area palette-open"
            : "canvas-work-area palette-closed"
        }
      >
        {editable && paletteOpen ? (
          <div id="block-palette">
            <BlockPalette
              templates={blockTemplates}
              onAdd={addFromPalette}
            />
          </div>
        ) : null}

        <div className="canvas-surface" ref={surfaceRef}>
          <ReactFlow
            aria-label="Algoram graph canvas"
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            onNodesChange={handleNodesChange}
            onNodeDragStart={() => {
              pointerDragActive.current = true;
              markEditorPerformance("node-drag-start");
            }}
            onNodeDragStop={(_, node) => {
              pointerDragActive.current = false;
              commitNodePosition(node);
              markEditorPerformance("node-drag-stop");
            }}
            onConnect={connectCanonical}
            isValidConnection={isValidConnection}
            onNodeClick={(_, node) => {
              onSelectConnection(null);
              onSelectBlock(node.id);
            }}
            onEdgeClick={(_, edge) => {
              if (!edge.id.startsWith("draft:")) {
                onSelectBlock(null);
                onSelectConnection(edge.id);
              }
            }}
            onNodeDoubleClick={(_, node) => {
              const reference = node.data.block.internal_graph_ref;
              if (reference) {
                onOpenGraph(reference, node.data.block);
              }
            }}
            onPaneClick={() => {
              onSelectBlock(null);
              onSelectConnection(null);
            }}
            nodesConnectable={editable}
            nodesDraggable
            nodesFocusable
            edgesFocusable
            onlyRenderVisibleElements
            autoPanOnNodeFocus={false}
            connectOnClick={editable}
            disableKeyboardA11y={false}
            ariaLabelConfig={ariaLabelConfig}
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
