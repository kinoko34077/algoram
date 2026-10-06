import type {
  AlgoramGraph,
  ReferenceBundle,
  SourceArtifact,
} from "./algoram";

export interface NavigationRecord {
  blockId: string;
  label: string;
  graphId: string;
  graphLabel?: string;
  internalGraphRef?: string;
  sourceArtifactId?: string;
  sourceOrigin?: string;
  sourceLanguage?: string;
  semanticKey?: string;
}

export interface NavigationContainmentEdge {
  parentGraphId: string;
  childGraphId: string;
  viaBlockId: string;
  viaBlockLabel: string;
}

export interface NavigationIndex {
  rootGraphId: string;
  graphLabels: Record<string, string>;
  records: NavigationRecord[];
  containmentEdges: NavigationContainmentEdge[];
}

export interface NavigationPathEntry {
  graphId: string;
  label: string;
  viaBlockId?: string;
}

export interface NavigationSearchOptions {
  language?: string;
}

function artifactMap(bundle: ReferenceBundle): Map<string, SourceArtifact> {
  const artifacts = new Map<string, SourceArtifact>();

  for (const graph of Object.values(bundle.graphs)) {
    for (const artifact of graph.source_artifacts ?? []) {
      if (!artifacts.has(artifact.id)) {
        artifacts.set(artifact.id, artifact);
      }
    }
  }

  for (const document of Object.values(bundle.sources)) {
    artifacts.set(document.artifact.id, document.artifact);
  }

  return artifacts;
}

function graphLabel(graph: AlgoramGraph): string {
  return graph.label?.trim() || graph.id;
}

export function buildNavigationIndex(bundle: ReferenceBundle): NavigationIndex {
  const artifacts = artifactMap(bundle);
  const graphLabels: Record<string, string> = {};
  const records: NavigationRecord[] = [];
  const containmentEdges: NavigationContainmentEdge[] = [];

  const graphIds = Object.keys(bundle.graphs).sort(compareText);

  for (const graphId of graphIds) {
    const graph = bundle.graphs[graphId];
    graphLabels[graphId] = graphLabel(graph);

    for (const block of graph.blocks) {
      const sourceArtifactId = block.source_anchor?.artifact_id;
      const artifact =
        sourceArtifactId === undefined
          ? undefined
          : artifacts.get(sourceArtifactId);

      records.push({
        blockId: block.id,
        label: block.label,
        graphId,
        graphLabel: graph.label,
        internalGraphRef: block.internal_graph_ref,
        sourceArtifactId,
        sourceOrigin: artifact?.origin,
        sourceLanguage: artifact?.language,
        semanticKey: block.source_anchor?.semantic_key,
      });

      if (block.internal_graph_ref) {
        containmentEdges.push({
          parentGraphId: graphId,
          childGraphId: block.internal_graph_ref,
          viaBlockId: block.id,
          viaBlockLabel: block.label,
        });
      }
    }
  }

  records.sort(compareRecords);
  containmentEdges.sort(compareEdges);

  return {
    rootGraphId: bundle.rootGraphId,
    graphLabels,
    records,
    containmentEdges,
  };
}

function compareText(left: string, right: string): number {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

function compareRecords(
  left: NavigationRecord,
  right: NavigationRecord,
): number {
  return (
    compareText(left.label, right.label) ||
    compareText(left.graphId, right.graphId) ||
    compareText(left.blockId, right.blockId)
  );
}

function compareEdges(
  left: NavigationContainmentEdge,
  right: NavigationContainmentEdge,
): number {
  return (
    compareText(left.parentGraphId, right.parentGraphId) ||
    compareText(left.childGraphId, right.childGraphId) ||
    compareText(left.viaBlockId, right.viaBlockId)
  );
}

function searchableText(record: NavigationRecord): string[] {
  return [
    record.label,
    record.blockId,
    record.sourceOrigin,
    record.semanticKey,
  ]
    .filter((value): value is string => value !== undefined)
    .map((value) => value.toLowerCase());
}

export function searchNavigation(
  index: NavigationIndex,
  query: string,
  options: NavigationSearchOptions = {},
): NavigationRecord[] {
  const normalizedQuery = query.trim().toLowerCase();
  const language = options.language?.trim().toLowerCase();

  return index.records
    .filter((record) => {
      if (
        language &&
        record.sourceLanguage?.toLowerCase() !== language
      ) {
        return false;
      }

      if (!normalizedQuery) {
        return true;
      }

      return searchableText(record).some((value) =>
        value.includes(normalizedQuery),
      );
    })
    .sort(compareRecords);
}

export function resolveGraphPath(
  index: NavigationIndex,
  targetGraphId: string,
): NavigationPathEntry[] | null {
  const rootGraphId = index.rootGraphId;

  if (!index.graphLabels[rootGraphId] || !index.graphLabels[targetGraphId]) {
    return null;
  }

  if (targetGraphId === rootGraphId) {
    return [
      {
        graphId: rootGraphId,
        label: index.graphLabels[rootGraphId],
      },
    ];
  }

  const outgoing = new Map<string, NavigationContainmentEdge[]>();
  for (const edge of index.containmentEdges) {
    if (!index.graphLabels[edge.childGraphId]) {
      continue;
    }

    const edges = outgoing.get(edge.parentGraphId) ?? [];
    edges.push(edge);
    outgoing.set(edge.parentGraphId, edges);
  }
  for (const edges of outgoing.values()) {
    edges.sort(compareEdges);
  }

  interface QueueItem {
    graphId: string;
    path: NavigationPathEntry[];
  }

  const queue: QueueItem[] = [
    {
      graphId: rootGraphId,
      path: [
        {
          graphId: rootGraphId,
          label: index.graphLabels[rootGraphId],
        },
      ],
    },
  ];
  const visited = new Set<string>([rootGraphId]);

  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const current = queue[cursor];

    for (const edge of outgoing.get(current.graphId) ?? []) {
      if (visited.has(edge.childGraphId)) {
        continue;
      }

      const path = [
        ...current.path,
        {
          graphId: edge.childGraphId,
          label:
            edge.viaBlockLabel ||
            index.graphLabels[edge.childGraphId] ||
            edge.childGraphId,
          viaBlockId: edge.viaBlockId,
        },
      ];

      if (edge.childGraphId === targetGraphId) {
        return path;
      }

      visited.add(edge.childGraphId);
      queue.push({
        graphId: edge.childGraphId,
        path,
      });
    }
  }

  return null;
}

export function resolveBlockPath(
  index: NavigationIndex,
  record: NavigationRecord,
): NavigationPathEntry[] | null {
  return resolveGraphPath(index, record.graphId);
}
