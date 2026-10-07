import type { AlgoramGraph } from "./algoram";
import {
  EMPTY_GRAPH_PRESENTATION,
  type GraphPresentationState,
  type ViewportState,
} from "./presentation";

export const EDITOR_SESSION_SCHEMA_VERSION = "algoram.editor-session/0.1";

export interface PersistedGraphPresentation {
  positions: GraphPresentationState["positions"];
  viewport?: ViewportState;
}

export interface PersistedEditorSession {
  schema_version: typeof EDITOR_SESSION_SCHEMA_VERSION;
  root_graph_id: string;
  graph_path: string[];
  presentations: Record<string, PersistedGraphPresentation>;
  selected_block_ids: Record<string, string>;
  selected_connection_ids: Record<string, string>;
  inspector_open: boolean;
}

export interface RestoredEditorSession {
  graphPath: string[];
  presentations: Record<string, GraphPresentationState>;
  selectedBlockIds: Record<string, string | null>;
  selectedConnectionIds: Record<string, string | null>;
  inspectorOpen: boolean;
}

function finite(value: number): boolean {
  return Number.isFinite(value);
}

function validViewport(viewport: ViewportState | undefined): viewport is ViewportState {
  return Boolean(
    viewport &&
      finite(viewport.x) &&
      finite(viewport.y) &&
      finite(viewport.zoom) &&
      viewport.zoom > 0,
  );
}

function graphChildRefs(graph: AlgoramGraph): Set<string> {
  return new Set(
    graph.blocks
      .map((block) => block.internal_graph_ref)
      .filter((value): value is string => typeof value === "string"),
  );
}

function sanitizePath(
  rootGraphId: string,
  graphs: Record<string, AlgoramGraph>,
  path: string[],
): string[] {
  const result = [rootGraphId];
  let current = graphs[rootGraphId];

  if (!current) {
    return result;
  }

  for (const nextId of path.slice(1)) {
    if (!graphs[nextId] || !graphChildRefs(current).has(nextId)) {
      break;
    }
    result.push(nextId);
    current = graphs[nextId];
  }

  return result;
}

export function createPersistedEditorSession(
  rootGraphId: string,
  graphs: Record<string, AlgoramGraph>,
  graphPath: string[],
  presentations: Record<string, GraphPresentationState>,
  selectedBlockIds: Record<string, string | null>,
  selectedConnectionIds: Record<string, string | null>,
  inspectorOpen: boolean,
): PersistedEditorSession {
  const stablePresentations: Record<string, PersistedGraphPresentation> = {};
  const stableBlocks: Record<string, string> = {};
  const stableConnections: Record<string, string> = {};

  for (const [graphId, graph] of Object.entries(graphs)) {
    const presentation = presentations[graphId];
    if (presentation) {
      const blockIds = new Set(graph.blocks.map((block) => block.id));
      const positions = Object.fromEntries(
        Object.entries(presentation.positions).filter(
          ([blockId, position]) =>
            blockIds.has(blockId) && finite(position.x) && finite(position.y),
        ),
      );

      stablePresentations[graphId] = {
        positions,
        ...(validViewport(presentation.viewport)
          ? { viewport: { ...presentation.viewport } }
          : {}),
      };
    }

    const blockId = selectedBlockIds[graphId];
    if (blockId && graph.blocks.some((block) => block.id === blockId)) {
      stableBlocks[graphId] = blockId;
    }

    const connectionId = selectedConnectionIds[graphId];
    if (
      connectionId &&
      graph.connections.some((connection) => connection.id === connectionId)
    ) {
      stableConnections[graphId] = connectionId;
    }
  }

  return {
    schema_version: EDITOR_SESSION_SCHEMA_VERSION,
    root_graph_id: rootGraphId,
    graph_path: sanitizePath(rootGraphId, graphs, graphPath),
    presentations: stablePresentations,
    selected_block_ids: stableBlocks,
    selected_connection_ids: stableConnections,
    inspector_open: inspectorOpen,
  };
}

export function restoreEditorSession(
  rootGraph: AlgoramGraph,
  graphs: Record<string, AlgoramGraph>,
  session: PersistedEditorSession | null,
): RestoredEditorSession {
  const availableGraphs = {
    ...graphs,
    [rootGraph.id]: rootGraph,
  };

  if (
    !session ||
    session.schema_version !== EDITOR_SESSION_SCHEMA_VERSION ||
    session.root_graph_id !== rootGraph.id
  ) {
    return {
      graphPath: [rootGraph.id],
      presentations: {},
      selectedBlockIds: {},
      selectedConnectionIds: {},
      inspectorOpen: true,
    };
  }

  const presentations: Record<string, GraphPresentationState> = {};
  for (const [graphId, persisted] of Object.entries(session.presentations ?? {})) {
    const graph = availableGraphs[graphId];
    if (!graph) {
      continue;
    }

    const blockIds = new Set(graph.blocks.map((block) => block.id));
    const positions = Object.fromEntries(
      Object.entries(persisted.positions ?? {}).filter(
        ([blockId, position]) =>
          blockIds.has(blockId) &&
          finite(position.x) &&
          finite(position.y),
      ),
    );

    presentations[graphId] = {
      ...EMPTY_GRAPH_PRESENTATION,
      positions,
      ...(validViewport(persisted.viewport)
        ? { viewport: { ...persisted.viewport } }
        : {}),
    };
  }

  const selectedBlockIds: Record<string, string | null> = {};
  for (const [graphId, blockId] of Object.entries(session.selected_block_ids ?? {})) {
    const graph = availableGraphs[graphId];
    if (graph?.blocks.some((block) => block.id === blockId)) {
      selectedBlockIds[graphId] = blockId;
    }
  }

  const selectedConnectionIds: Record<string, string | null> = {};
  for (const [graphId, connectionId] of Object.entries(
    session.selected_connection_ids ?? {},
  )) {
    const graph = availableGraphs[graphId];
    if (graph?.connections.some((connection) => connection.id === connectionId)) {
      selectedConnectionIds[graphId] = connectionId;
    }
  }

  return {
    graphPath: sanitizePath(
      rootGraph.id,
      availableGraphs,
      session.graph_path ?? [rootGraph.id],
    ),
    presentations,
    selectedBlockIds,
    selectedConnectionIds,
    inspectorOpen: session.inspector_open !== false,
  };
}
