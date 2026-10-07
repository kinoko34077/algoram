export type PortDirection = "in" | "out";
export type PortChannel = "flow" | "data";

export interface AlgoramPort {
  id: string;
  direction: PortDirection;
  channel: PortChannel;
  contract?: unknown;
  extensions?: Record<string, unknown>;
}

export interface SourceAnchor {
  artifact_id: string;
  start_byte: number;
  end_byte: number;
  start_line?: number;
  start_column?: number;
  end_line?: number;
  end_column?: number;
  semantic_key?: string;
}

export interface SourceArtifact {
  id: string;
  origin: string;
  language: string;
  revision?: string;
  content_hash?: string;
  repository_origin?: string;
  license?: string;
}

export interface Diagnostic {
  code: string;
  message: string;
  source_anchor?: SourceAnchor;
}

export interface AlgoramBlock {
  id: string;
  label: string;
  ports?: AlgoramPort[];
  internal_graph_ref?: string;
  implementation_ref?: string;
  definition_ref?: string;
  source_anchor?: SourceAnchor;
  extensions?: Record<string, unknown>;
  diagnostics?: Diagnostic[];
}

export interface PortRef {
  block_id: string;
  port_id: string;
}

export interface AlgoramConnection {
  id: string;
  source: PortRef;
  target: PortRef;
  extensions?: Record<string, unknown>;
}

export interface AlgoramGraph {
  schema_version: string;
  id: string;
  label?: string;
  blocks: AlgoramBlock[];
  connections: AlgoramConnection[];
  source_artifacts?: SourceArtifact[];
  extensions?: Record<string, unknown>;
  presentation?: Record<string, unknown>;
  diagnostics?: Diagnostic[];
}

export interface SourceDocument {
  artifact: SourceArtifact;
  text: string;
}

export interface ReferenceBundle {
  rootGraphId: string;
  graphs: Record<string, AlgoramGraph>;
  sources: Record<string, SourceDocument>;
  routeInspections?: Record<string, string>;
  editableGraphIds?: string[];
}
