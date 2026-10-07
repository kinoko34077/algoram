import type { AlgoramGraph } from "./algoram";

export interface ExportValidationIssue {
  message: string;
}

export type GraphExportValidator = (
  graph: AlgoramGraph,
) => readonly ExportValidationIssue[];

export interface CanonicalGraphExport {
  filename: string;
  json: string;
}

export class GraphExportValidationError extends Error {
  readonly issues: readonly ExportValidationIssue[];

  constructor(issues: readonly ExportValidationIssue[]) {
    super(issues[0]?.message ?? "Graph validation failed.");
    this.name = "GraphExportValidationError";
    this.issues = issues;
  }
}

function safeGraphId(graphId: string): string {
  const normalized = graphId
    .trim()
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^[._-]+|[._-]+$/g, "")
    .slice(0, 120);

  return normalized || "graph";
}

export function graphExportFilename(graph: AlgoramGraph): string {
  return `algoram-${safeGraphId(graph.id)}.algoram.json`;
}

export function serializeCanonicalGraph(graph: AlgoramGraph): string {
  return `${JSON.stringify(graph, null, 2)}\n`;
}

export function prepareCanonicalGraphExport(
  graph: AlgoramGraph,
  validate: GraphExportValidator,
): CanonicalGraphExport {
  const issues = validate(graph);
  if (issues.length > 0) {
    throw new GraphExportValidationError(issues);
  }

  return {
    filename: graphExportFilename(graph),
    json: serializeCanonicalGraph(graph),
  };
}
