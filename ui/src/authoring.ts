import type { AlgoramGraph, ReferenceBundle } from "./algoram";
import { validateGraph, type GraphValidationIssue } from "./graphValidation";

interface GraphRevision {
  graph: AlgoramGraph;
  fingerprint: string;
  revision: number;
}

export interface GraphHistory {
  past: GraphRevision[];
  present: GraphRevision;
  future: GraphRevision[];
  savedFingerprint: string;
  nextRevision: number;
}

export type AuthoringByGraph = Record<string, GraphHistory>;

function sortForFingerprint(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortForFingerprint);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, sortForFingerprint(nested)]),
    );
  }
  return value;
}

export function graphFingerprint(graph: AlgoramGraph): string {
  return JSON.stringify(sortForFingerprint(graph));
}

export function cloneGraph(graph: AlgoramGraph): AlgoramGraph {
  return structuredClone(graph);
}

export function createGraphHistory(graph: AlgoramGraph): GraphHistory {
  const workingGraph = cloneGraph(graph);
  const fingerprint = graphFingerprint(workingGraph);
  return {
    past: [],
    present: {
      graph: workingGraph,
      fingerprint,
      revision: 0,
    },
    future: [],
    savedFingerprint: fingerprint,
    nextRevision: 1,
  };
}

export function createAuthoringHistories(
  bundle: ReferenceBundle,
): AuthoringByGraph {
  const histories: AuthoringByGraph = {};
  for (const graphId of bundle.editableGraphIds ?? []) {
    const graph = bundle.graphs[graphId];
    if (graph) {
      histories[graphId] = createGraphHistory(graph);
    }
  }
  return histories;
}

export function graphIsDirty(history: GraphHistory): boolean {
  return history.present.fingerprint !== history.savedFingerprint;
}

export function applyGraph(
  history: GraphHistory,
  nextGraph: AlgoramGraph,
): GraphHistory {
  const fingerprint = graphFingerprint(nextGraph);
  if (fingerprint === history.present.fingerprint) {
    return history;
  }

  return {
    past: [...history.past, history.present],
    present: {
      graph: nextGraph,
      fingerprint,
      revision: history.nextRevision,
    },
    future: [],
    savedFingerprint: history.savedFingerprint,
    nextRevision: history.nextRevision + 1,
  };
}

export function undoGraph(history: GraphHistory): GraphHistory {
  const previous = history.past.at(-1);
  if (!previous) {
    return history;
  }

  return {
    ...history,
    past: history.past.slice(0, -1),
    present: previous,
    future: [history.present, ...history.future],
  };
}

export function redoGraph(history: GraphHistory): GraphHistory {
  const next = history.future[0];
  if (!next) {
    return history;
  }

  return {
    ...history,
    past: [...history.past, history.present],
    present: next,
    future: history.future.slice(1),
  };
}

export function markGraphSaved(history: GraphHistory): GraphHistory {
  if (history.savedFingerprint === history.present.fingerprint) {
    return history;
  }

  return {
    ...history,
    savedFingerprint: history.present.fingerprint,
  };
}

export function setGraphLabel(
  graph: AlgoramGraph,
  label: string,
): AlgoramGraph {
  const nextLabel = label.trim().length === 0 ? undefined : label;
  if (graph.label === nextLabel) {
    return graph;
  }

  return {
    ...graph,
    label: nextLabel,
  };
}

export function validateWorkingGraph(
  history: GraphHistory,
): GraphValidationIssue[] {
  return validateGraph(history.present.graph);
}

export function isNativeEditableGraph(
  bundle: ReferenceBundle,
  graphId: string,
): boolean {
  return (bundle.editableGraphIds ?? []).includes(graphId);
}
