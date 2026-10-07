import type { AlgoramGraph } from "./algoram";

export interface NodePosition {
  x: number;
  y: number;
}

export interface ViewportState {
  x: number;
  y: number;
  zoom: number;
}

export interface DraftLink {
  id: string;
  sourceBlockId: string;
  sourcePortId: string;
  targetBlockId: string;
  targetPortId: string;
}

export interface GraphPresentationState {
  positions: Record<string, NodePosition>;
  draftLinks: DraftLink[];
  viewport?: ViewportState;
}

export const EMPTY_GRAPH_PRESENTATION: GraphPresentationState = {
  positions: {},
  draftLinks: [],
};

export function draftLinkId(
  sourceBlockId: string,
  sourcePortId: string,
  targetBlockId: string,
  targetPortId: string,
): string {
  return `draft:${sourceBlockId}:${sourcePortId}->${targetBlockId}:${targetPortId}`;
}

export function makeDraftLink(
  sourceBlockId: string,
  sourcePortId: string,
  targetBlockId: string,
  targetPortId: string,
): DraftLink {
  return {
    id: draftLinkId(
      sourceBlockId,
      sourcePortId,
      targetBlockId,
      targetPortId,
    ),
    sourceBlockId,
    sourcePortId,
    targetBlockId,
    targetPortId,
  };
}

function findPort(graph: AlgoramGraph, blockId: string, portId: string) {
  return graph.blocks
    .find((block) => block.id === blockId)
    ?.ports?.find((port) => port.id === portId);
}

export function validateDraftLink(
  graph: AlgoramGraph,
  state: GraphPresentationState,
  link: DraftLink,
): string | null {
  const source = findPort(graph, link.sourceBlockId, link.sourcePortId);
  const target = findPort(graph, link.targetBlockId, link.targetPortId);

  if (!source || source.direction !== "out") {
    return "Choose an output port that exists in the current Graph.";
  }

  if (!target || target.direction !== "in") {
    return "Choose an input port that exists in the current Graph.";
  }

  if (source.channel !== target.channel) {
    return "Draft links must keep the same flow/data channel.";
  }

  const canonicalDuplicate = graph.connections.some(
    (connection) =>
      connection.source.block_id === link.sourceBlockId &&
      connection.source.port_id === link.sourcePortId &&
      connection.target.block_id === link.targetBlockId &&
      connection.target.port_id === link.targetPortId,
  );
  if (canonicalDuplicate) {
    return "That connection already exists in the canonical Graph.";
  }

  if (state.draftLinks.some((item) => item.id === link.id)) {
    return "That draft link already exists.";
  }

  return null;
}

export function addDraftLink(
  graph: AlgoramGraph,
  state: GraphPresentationState,
  link: DraftLink,
): GraphPresentationState {
  if (validateDraftLink(graph, state, link)) {
    return state;
  }

  return {
    ...state,
    draftLinks: [...state.draftLinks, link],
  };
}

export function clearDraftLinks(
  state: GraphPresentationState,
): GraphPresentationState {
  if (state.draftLinks.length === 0) {
    return state;
  }

  return {
    ...state,
    draftLinks: [],
  };
}

export function setNodePosition(
  state: GraphPresentationState,
  blockId: string,
  position: NodePosition,
): GraphPresentationState {
  const current = state.positions[blockId];
  if (current?.x === position.x && current.y === position.y) {
    return state;
  }

  return {
    ...state,
    positions: {
      ...state.positions,
      [blockId]: position,
    },
  };
}

export function resetNodePositions(
  state: GraphPresentationState,
): GraphPresentationState {
  if (Object.keys(state.positions).length === 0) {
    return state;
  }

  return {
    ...state,
    positions: {},
  };
}

export function setPresentationViewport(
  state: GraphPresentationState,
  viewport: ViewportState,
): GraphPresentationState {
  const current = state.viewport;
  if (
    current?.x === viewport.x &&
    current.y === viewport.y &&
    current.zoom === viewport.zoom
  ) {
    return state;
  }

  return {
    ...state,
    viewport: { ...viewport },
  };
}

export function clearPresentationViewport(
  state: GraphPresentationState,
): GraphPresentationState {
  if (!state.viewport) {
    return state;
  }

  const { viewport: _viewport, ...rest } = state;
  return rest;
}
