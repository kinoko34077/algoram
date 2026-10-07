import type { AlgoramConnection, AlgoramGraph } from "./algoram";

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

function findPort(
  graph: AlgoramGraph,
  blockId: string,
  portId: string,
) {
  return graph.blocks
    .find((block) => block.id === blockId)
    ?.ports?.find((port) => port.id === portId);
}

export function validateConnectionCandidate(
  graph: AlgoramGraph,
  candidate: ConnectionCandidate,
): string | null {
  const connection = makeCanonicalConnection(graph, candidate);
  const sourceBlock = graph.blocks.find(
    (block) => block.id === candidate.sourceBlockId,
  );
  if (!sourceBlock) {
    return `Connection '${connection.id}' references missing Block '${candidate.sourceBlockId}'.`;
  }

  const targetBlock = graph.blocks.find(
    (block) => block.id === candidate.targetBlockId,
  );
  if (!targetBlock) {
    return `Connection '${connection.id}' references missing Block '${candidate.targetBlockId}'.`;
  }

  const source = findPort(
    graph,
    candidate.sourceBlockId,
    candidate.sourcePortId,
  );
  if (!source) {
    return (
      `Connection '${connection.id}' references missing Port ` +
      `'${candidate.sourceBlockId}.${candidate.sourcePortId}'.`
    );
  }

  const target = findPort(
    graph,
    candidate.targetBlockId,
    candidate.targetPortId,
  );
  if (!target) {
    return (
      `Connection '${connection.id}' references missing Port ` +
      `'${candidate.targetBlockId}.${candidate.targetPortId}'.`
    );
  }

  if (source.direction !== "out" || target.direction !== "in") {
    return (
      `Connection '${connection.id}' requires out → in, got ` +
      `${source.direction} → ${target.direction}.`
    );
  }

  if (source.channel !== target.channel) {
    return (
      `Connection '${connection.id}' channel mismatch: ` +
      `${source.channel} → ${target.channel}.`
    );
  }

  return null;
}

export function createConnection(
  graph: AlgoramGraph,
  candidate: ConnectionCandidate,
): CreateConnectionResult {
  const reason = validateConnectionCandidate(graph, candidate);
  if (reason) {
    return {
      ok: false,
      graph,
      reason,
    };
  }

  const connection = makeCanonicalConnection(graph, candidate);
  return {
    ok: true,
    graph: {
      ...graph,
      connections: [...graph.connections, connection],
    },
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
