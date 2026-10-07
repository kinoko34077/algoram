import type { AlgoramConnection, AlgoramGraph } from "./algoram";
import { validateGraph } from "./graphValidation";

export interface ConnectionCandidate {
  sourceBlockId: string;
  sourcePortId: string;
  targetBlockId: string;
  targetPortId: string;
}

export type CreateConnectionResult =
  | {
      ok: true;
      graph: AlgoramGraph;
      connection: AlgoramConnection;
    }
  | {
      ok: false;
      graph: AlgoramGraph;
      reason: string;
    };

export type RemoveConnectionResult =
  | {
      ok: true;
      graph: AlgoramGraph;
      removed: AlgoramConnection;
    }
  | {
      ok: false;
      graph: AlgoramGraph;
      reason: string;
    };

export function nextConnectionId(graph: AlgoramGraph): string {
  const occupied = new Set(graph.connections.map((connection) => connection.id));
  for (let sequence = 1; ; sequence += 1) {
    const candidate = `connection:placed:${sequence}`;
    if (!occupied.has(candidate)) {
      return candidate;
    }
  }
}

export function makeCanonicalConnection(
  graph: AlgoramGraph,
  candidate: ConnectionCandidate,
): AlgoramConnection {
  return {
    id: nextConnectionId(graph),
    source: {
      block_id: candidate.sourceBlockId,
      port_id: candidate.sourcePortId,
    },
    target: {
      block_id: candidate.targetBlockId,
      port_id: candidate.targetPortId,
    },
  };
}

export function validateConnectionCandidate(
  graph: AlgoramGraph,
  candidate: ConnectionCandidate,
): string | null {
  const connection = makeCanonicalConnection(graph, candidate);
  const nextGraph: AlgoramGraph = {
    ...graph,
    connections: [...graph.connections, connection],
  };
  const issue = validateGraph(nextGraph)[0];
  return issue?.message ?? null;
}

export function createConnection(
  graph: AlgoramGraph,
  candidate: ConnectionCandidate,
): CreateConnectionResult {
  const connection = makeCanonicalConnection(graph, candidate);
  const nextGraph: AlgoramGraph = {
    ...graph,
    connections: [...graph.connections, connection],
  };
  const issue = validateGraph(nextGraph)[0];
  if (issue) {
    return {
      ok: false,
      graph,
      reason: issue.message,
    };
  }

  return {
    ok: true,
    graph: nextGraph,
    connection,
  };
}

export function removeConnection(
  graph: AlgoramGraph,
  connectionId: string,
): RemoveConnectionResult {
  const connection = graph.connections.find(
    (candidate) => candidate.id === connectionId,
  );
  if (!connection) {
    return {
      ok: false,
      graph,
      reason: `Connection '${connectionId}' no longer exists.`,
    };
  }

  return {
    ok: true,
    graph: {
      ...graph,
      connections: graph.connections.filter(
        (candidate) => candidate.id !== connectionId,
      ),
    },
    removed: structuredClone(connection),
  };
}
