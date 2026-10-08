import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MiniMap,
  NodeToolbar,
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
  type Viewport,
  type XYPosition,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { jaJP } from "./locales/ja-JP";
import type { AlgoramBlock, AlgoramGraph } from "./algoram";
import { BlockPalette } from "./BlockPalette";
import { FirstUseGuide, useFirstUseGuide } from "./FirstUseGuide";
import type { ReusableBlockTemplate } from "./blockAuthoring";
import { getBlockGeometry, portHandleTop, projectBlockDisplay } from "./blockDisplay";
import { Glossable } from "./Glossable";
import { NearBlockDetail } from "./NearBlockDetail";
import type { CapabilityAddSource } from "./BlockPalette";
import { markEditorPerformance } from "./perfMarks";
import {
  toFlowEdges,
  toFlowNodes,
  type FlowBlockNode,
} from "./graphAdapter";
import {
  clearDraftLinks,
  clearPresentationViewport,
  makeDraftLink,
  resetNodePositions,
  setNodePosition,
  setPresentationViewport,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";
import {
  observedStatusLabel,
  observedStatusSymbol,
  type BlockTraceObservation,
  type TraceProjection,
} from "./traceProjection";

interface GraphFocusRequest {
  blockId: string;
  revision: number;
}

interface GraphCanvasProps {
  graph: AlgoramGraph;
  editable: boolean;
  blockTemplates: ReusableBlockTemplate[];
  authoringStatus: string | null;
  traceProjection: TraceProjection | null;
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
  onShowInspector: () => void;
}

function contractLabel(contract: unknown): string {
  if (typeof contract === "string") return contract;
  if (contract === undefined) return "";
  try {
    return JSON.stringify(contract);
  } catch {
    return String(contract);
  }
}

function BlockNode({ data, selected }: NodeProps<FlowBlockNode>) {
  const { block, display } = data;
  const observation = data.traceObservation as
    | BlockTraceObservation
    | undefined;
  const ports = block.ports ?? [];
  const inputs = ports.filter((port) => port.direction === "in");
  const outputs = ports.filter((port) => port.direction === "out");

  const nodeClassName = [
    "algoram-node",
    selected ? "selected" : "",
    observation ? `observed-${observation.status}` : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={nodeClassName}>
      <NodeToolbar
        isVisible={selected}
        position={Position.Bottom}
        offset={16}
        className="near-block-toolbar"
      >
        <NearBlockDetail
          block={block}
          observation={observation}
          onOpenComposite={data.onOpenComposite}
          onShowInspector={data.onShowInspector}
        />
      </NodeToolbar>
      {inputs.map((port, index) => (
        <Handle
          key={port.id}
          id={port.id}
          type="target"
          position={Position.Left}
          className={`algoram-handle ${port.channel}`}
          style={{ top: portHandleTop(index, inputs.length, display.geometry.height) }}
          title={jaJP.authoring.canvas.inputPortLabel.replace("{channel}", port.channel).replace("{contract}", contractLabel(port.contract))}
          aria-label={`${block.label} ${port.id} 入力`}
        />
      ))}

      <div className="node-titlebar">
        <div className="node-title">
          <Glossable shortLabel={display.shortLabel} fullLabel={display.gloss} />
        </div>
        {block.internal_graph_ref ? (
          <span className="node-depth-indicator" aria-hidden="true">↳</span>
        ) : null}
      </div>

      {observation || block.diagnostics?.length ? (
        <div className="node-meta">
          {observation ? (
            <span
              className={`observed-status ${observation.status}`}
              title={jaJP.authoring.canvas.observedRun.replace("{status}", observedStatusLabel(observation.status))}
            >
              <b aria-hidden="true">{observedStatusSymbol(observation.status)}</b>
              {observedStatusLabel(observation.status)}
            </span>
          ) : null}
          {block.diagnostics?.length ? (
            <span>{jaJP.authoring.canvas.diagnosticsCount.replace("{count}", String(block.diagnostics.length))}</span>
          ) : null}
        </div>
      ) : null}

      {selected && data.onOpenLocalAdd && outputs.length > 0 ? (
        <button
          type="button"
          className="node-local-add nodrag nopan"
          title={jaJP.authoring.canvas.localPlus}
          aria-label={jaJP.authoring.canvas.localPlus}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => {
            event.stopPropagation();
            data.onOpenLocalAdd?.(block.id);
          }}
        >
          ＋
        </button>
      ) : null}

      {outputs.map((port, index) => (
        <Handle
          key={port.id}
          id={port.id}
          type="source"
          position={Position.Right}
          className={`algoram-handle ${port.channel}`}
          style={{ top: portHandleTop(index, outputs.length, display.geometry.height) }}
          title={jaJP.authoring.canvas.outputPortLabel.replace("{channel}", port.channel).replace("{contract}", contractLabel(port.contract))}
          aria-label={`${block.label} ${port.id} 出力`}
        />
      ))}
    </div>
  );
}

const nodeTypes: NodeTypes = {
  algoramBlock: BlockNode,
};

const nodeA11yDescription =
  jaJP.authoring.canvas.keyboardNodeHint;

const ariaLabelConfig = {
  "node.a11yDescription.default": nodeA11yDescription,
  "node.a11yDescription.keyboardDisabled": nodeA11yDescription,
  "edge.a11yDescription.default":
    jaJP.authoring.canvas.connectionAccessibilityHint,
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
    label: jaJP.authoring.canvas.draftLabel,
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
  traceProjection,
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
  onShowInspector,
}: GraphCanvasProps) {
  const [baseNodes, setBaseNodes, onNodesChange] =
    useNodesState<FlowBlockNode>([]);
  const [layoutError, setLayoutError] = useState<string | null>(null);
  const [interactionStatus, setInteractionStatus] = useState<string | null>(
    null,
  );
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [addContext, setAddContext] = useState<{
    blockId: string;
    sourcePortId: string | null;
  } | null>(null);
  const firstUse = useFirstUseGuide();
  const [blockDropActive, setBlockDropActive] = useState(false);
  const pointerDragActive = useRef(false);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [loadedGraphId, setLoadedGraphId] = useState<string | null>(null);
  const [loadedStructureKey, setLoadedStructureKey] = useState<string | null>(
    null,
  );
  const { fitView, getNode, screenToFlowPosition, setViewport } =
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
            if (useSavedPositions && presentation.viewport) {
              void setViewport(presentation.viewport, { duration: 0 });
            } else {
              void fitView({ padding: 0.2, duration: 160 });
            }
          });
        }
      } catch (error: unknown) {
        setLayoutError(
          error instanceof Error ? error.message : jaJP.authoring.canvas.graphLayoutFailed,
        );
      }
    },
    [
      fitView,
      graph,
      presentation.positions,
      presentation.viewport,
      setBaseNodes,
      setViewport,
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

  const openLocalAdd = useCallback(
    (blockId: string) => {
      if (!editable) return;
      const block = graph.blocks.find((item) => item.id === blockId);
      const outputs = (block?.ports ?? []).filter((port) => port.direction === "out");
      if (!outputs.length) return;
      setAddContext({
        blockId,
        sourcePortId: outputs.length === 1 ? outputs[0].id : null,
      });
      setPaletteOpen(true);
      setInteractionStatus(null);
    },
    [editable, graph],
  );

  const localAddSource = useMemo<CapabilityAddSource | null>(() => {
    if (!addContext) return null;
    const block = graph.blocks.find((item) => item.id === addContext.blockId);
    if (!block) return null;
    return {
      blockId: block.id,
      blockLabel: projectBlockDisplay(block).shortLabel,
      outputs: (block.ports ?? []).filter((port) => port.direction === "out"),
      sourcePortId: addContext.sourcePortId,
    };
  }, [addContext, graph]);

  const nodes = useMemo(
    () =>
      baseNodes.map((node) => ({
        ...node,
        selected: node.id === selectedBlockId,
        data: {
          ...node.data,
          traceObservation: traceProjection?.byBlockId[node.id],
          onOpenLocalAdd: editable ? openLocalAdd : undefined,
          onOpenComposite: onOpenGraph,
          onShowInspector,
        },
      })),
    [baseNodes, selectedBlockId, traceProjection, editable, openLocalAdd, onOpenGraph, onShowInspector],
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
        setInteractionStatus(jaJP.authoring.canvas.choosePorts);
        return;
      }

      const error = onAddConnection(link);
      setInteractionStatus(error ?? jaJP.authoring.canvas.canonicalConnectionCreated);
    },
    [editable, onAddConnection],
  );

  const resetLayout = useCallback(() => {
    onPresentationChange((current) =>
      clearPresentationViewport(resetNodePositions(current)),
    );
    void loadLayout(false, true, false);
    setInteractionStatus(jaJP.authoring.canvas.layoutReset);
  }, [loadLayout, onPresentationChange]);

  const clearDrafts = useCallback(() => {
    onPresentationChange(clearDraftLinks);
    setInteractionStatus(jaJP.authoring.canvas.draftsCleared);
  }, [onPresentationChange]);

  useEffect(() => {
    if (authoringStatus) {
      setInteractionStatus(null);
    }
  }, [authoringStatus]);

  const addFromPalette = useCallback(
    (template: ReusableBlockTemplate) => {
      const surface = surfaceRef.current;
      const fallback = { x: 40, y: 40 };
      let position = fallback;

      const { width, height } = getBlockGeometry(template.block);
      const sourceNode = addContext?.sourcePortId
        ? getNode(addContext.blockId)
        : null;
      if (sourceNode) {
        const sourceHeight = getBlockGeometry(sourceNode.data.block).height;
        const sourceWidth = getBlockGeometry(sourceNode.data.block).width;
        position = {
          x: sourceNode.position.x + sourceWidth + 72,
          y: sourceNode.position.y + (sourceHeight - height) / 2,
        };
      } else if (surface) {
        const bounds = surface.getBoundingClientRect();
        const center = screenToFlowPosition({
          x: bounds.left + bounds.width / 2,
          y: bounds.top + bounds.height / 2,
        });
        position = { x: center.x - width / 2, y: center.y - height / 2 };
      }
      if (addContext && !addContext.sourcePortId) {
        setInteractionStatus(jaJP.authoring.blockPalette.outputChoice);
        return;
      }
      onAddBlock(template, position);
      if (addContext) {
        setInteractionStatus(jaJP.authoring.blockPalette.addedWithoutRoute);
      }
      setPaletteOpen(false);
      firstUse.dismiss();
    },
    [addContext, getNode, onAddBlock, screenToFlowPosition, firstUse.dismiss],
  );

  return (
    <div className="graph-canvas">
      <div className="canvas-toolbar" aria-label={jaJP.authoring.canvas.nodeWorkspaceControls}>
        <div className="workspace-mode">
          <strong>{jaJP.authoring.canvas.nodeWorkspace}</strong>
          {!editable ? <span>{jaJP.authoring.canvas.readOnlyMode}</span> : null}
        </div>

        <div className="canvas-toolbar-actions">
          {editable ? (
            <button
              type="button"
              className="tertiary-action"
              aria-controls="block-palette"
              aria-expanded={paletteOpen && addContext === null}
              aria-label={jaJP.authoring.canvas.globalPlus}
              title={jaJP.authoring.canvas.globalPlus}
              onClick={() => {
                setAddContext(null);
                setPaletteOpen((open) => addContext ? true : !open);
              }}
            >
              ＋
            </button>
          ) : null}
          <details className="canvas-more-menu">
            <summary className="tertiary-action" aria-label={jaJP.authoring.canvas.moreActions}>
              {jaJP.authoring.canvas.more}
            </summary>
            <div className="canvas-more-popover">
              <button type="button" className="tertiary-action"
                onClick={resetLayout} disabled={Object.keys(presentation.positions).length === 0}>
                {jaJP.authoring.canvas.resetLayoutAction}
              </button>
              <button type="button" className="tertiary-action"
                onClick={clearDrafts} disabled={presentation.draftLinks.length === 0}>
                {jaJP.authoring.canvas.clearDraftsAction}
              </button>
              <button type="button" className="tertiary-action" onClick={() => firstUse.reopen()}>
                {jaJP.authoring.canvas.showGuide}
              </button>
              <p>{editable ? jaJP.authoring.canvas.editableHint : jaJP.authoring.canvas.readOnlyHint}</p>
            </div>
          </details>
        </div>

        <div className="canvas-status" aria-live="polite">
          {presentation.draftLinks.length > 0 ? (<span>{jaJP.authoring.canvas.draftsCount.replace("{count}", String(presentation.draftLinks.length))}</span>) : null}
          {traceProjection ? (
            <span>
              {jaJP.authoring.canvas.observedBlocks.replace("{count}", String(Object.keys(traceProjection.byBlockId).length))}
              {traceProjection.unmappedOrigins.length > 0
                ? ` · ${jaJP.authoring.canvas.unmappedOrigins.replace("{count}", String(traceProjection.unmappedOrigins.length))}`
                : ""}
            </span>
          ) : null}
          {authoringStatus ? <span>{authoringStatus}</span> : null}
          {!authoringStatus && interactionStatus ? (
            <span>{interactionStatus}</span>
          ) : null}
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
              context={localAddSource}
              onSourcePortChange={(portId) => {
                setAddContext((current) => current ? { ...current, sourcePortId: portId } : null);
              }}
              onAdd={addFromPalette}
              onClose={() => setPaletteOpen(false)}
            />
          </div>
        ) : null}

        <div
          className={blockDropActive ? "canvas-surface block-drop-active" : "canvas-surface"}
          ref={surfaceRef}
          onDragOver={(event) => {
            if (!editable || !event.dataTransfer.types.includes("application/x-algoram-block-template")) return;
            event.preventDefault();
            event.dataTransfer.dropEffect = "copy";
            setBlockDropActive(true);
          }}
          onDragLeave={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              setBlockDropActive(false);
            }
          }}
          onDrop={(event) => {
            const definitionRef = event.dataTransfer.getData("application/x-algoram-block-template");
            if (!editable || !definitionRef) return;
            event.preventDefault();
            setBlockDropActive(false);
            const template = blockTemplates.find((item) => item.definitionRef === definitionRef);
            if (!template) {
              setInteractionStatus(jaJP.authoring.canvas.dropBlockUnavailable);
              return;
            }
            const center = screenToFlowPosition({ x: event.clientX, y: event.clientY });
            const { width, height } = getBlockGeometry(template.block);
            onAddBlock(template, { x: center.x - width / 2, y: center.y - height / 2 });
            setPaletteOpen(false);
            if (addContext) {
              setInteractionStatus(jaJP.authoring.blockPalette.addedWithoutRoute);
            }
            firstUse.dismiss();
          }}
        >
          <ReactFlow
            aria-label={jaJP.authoring.canvas.graphCanvas}
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
            onMoveEnd={(_, viewport: Viewport) => {
              onPresentationChange((current) =>
                setPresentationViewport(current, viewport),
              );
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
          {editable && firstUse.visible ? <FirstUseGuide onDismiss={firstUse.dismiss} /> : null}
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
