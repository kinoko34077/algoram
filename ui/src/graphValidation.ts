import type {
  AlgoramBlock,
  AlgoramGraph,
  AlgoramPort,
  PortRef,
} from "./algoram";
import { jaJP } from "./locales/ja-JP";
import { jaJP } from "./locales/ja-JP";

export const GRAPH_SCHEMA_VERSION = "algoram.graph/0.1";

export type GraphValidationCode =
  | "unsupported-schema-version"
  | "duplicate-source-artifact-id"
  | "duplicate-block-id"
  | "duplicate-port-id"
  | "invalid-source-anchor-range"
  | "missing-source-artifact"
  | "duplicate-connection-id"
  | "missing-block"
  | "missing-port"
  | "invalid-connection-direction"
  | "channel-mismatch";

export interface GraphValidationIssue {
  code: GraphValidationCode;
  message: string;
  graphId: string;
  blockId?: string;
  portId?: string;
  connectionId?: string;
}

function findPort(
  blocks: Map<string, AlgoramBlock>,
  reference: PortRef,
): AlgoramPort | undefined {
  return blocks
    .get(reference.block_id)
    ?.ports?.find((port) => port.id === reference.port_id);
}

export function validateGraph(graph: AlgoramGraph): GraphValidationIssue[] {
  const issues: GraphValidationIssue[] = [];

  if (graph.schema_version !== GRAPH_SCHEMA_VERSION) {
    issues.push({
      code: "unsupported-schema-version",
      graphId: graph.id,
      message: jaJP.errors.validation.unsupportedSchemaVersion
        .replace("{actual}", graph.schema_version)
        .replace("{expected}", GRAPH_SCHEMA_VERSION),
    });
  }

  const artifactIds = new Set<string>();
  for (const artifact of graph.source_artifacts ?? []) {
    if (artifactIds.has(artifact.id)) {
      issues.push({
        code: "duplicate-source-artifact-id",
        graphId: graph.id,
        message: jaJP.errors.validation.duplicateSourceArtifactId.replace("{artifactId}", artifact.id),
      });
    }
    artifactIds.add(artifact.id);
  }

  const blocks = new Map<string, AlgoramBlock>();
  for (const block of graph.blocks) {
    if (blocks.has(block.id)) {
      issues.push({
        code: "duplicate-block-id",
        graphId: graph.id,
        blockId: block.id,
        message: jaJP.errors.validation.duplicateBlockId.replace("{blockId}", block.id),
      });
    } else {
      blocks.set(block.id, block);
    }

    const portIds = new Set<string>();
    for (const port of block.ports ?? []) {
      if (portIds.has(port.id)) {
        issues.push({
          code: "duplicate-port-id",
          graphId: graph.id,
          blockId: block.id,
          portId: port.id,
          message: jaJP.errors.validation.duplicatePortId
            .replace("{portId}", port.id)
            .replace("{blockId}", block.id),
        });
      }
      portIds.add(port.id);
    }

    const anchor = block.source_anchor;
    if (anchor) {
      if (
        !Number.isSafeInteger(anchor.start_byte) ||
        !Number.isSafeInteger(anchor.end_byte) ||
        anchor.start_byte < 0 ||
        anchor.end_byte < anchor.start_byte
      ) {
        issues.push({
          code: "invalid-source-anchor-range",
          graphId: graph.id,
          blockId: block.id,
          message: jaJP.errors.validation.invalidSourceAnchorRange
            .replace("{blockId}", block.id)
            .replace("{start}", String(anchor.start_byte))
            .replace("{end}", String(anchor.end_byte)),
        });
      }
      if (!artifactIds.has(anchor.artifact_id)) {
        issues.push({
          code: "missing-source-artifact",
          graphId: graph.id,
          blockId: block.id,
          message: jaJP.errors.validation.missingSourceArtifact
            .replace("{blockId}", block.id)
            .replace("{artifactId}", anchor.artifact_id),
        });
      }
    }
  }

  const connectionIds = new Set<string>();
  for (const connection of graph.connections) {
    if (connectionIds.has(connection.id)) {
      issues.push({
        code: "duplicate-connection-id",
        graphId: graph.id,
        connectionId: connection.id,
        message: jaJP.errors.validation.duplicateConnectionId.replace("{connectionId}", connection.id),
      });
    }
    connectionIds.add(connection.id);

    const sourceBlock = blocks.get(connection.source.block_id);
    if (!sourceBlock) {
      issues.push({
        code: "missing-block",
        graphId: graph.id,
        connectionId: connection.id,
        blockId: connection.source.block_id,
        message: jaJP.errors.validation.missingSourceBlock
          .replace("{connectionId}", connection.id)
          .replace("{blockId}", connection.source.block_id),
      });
      continue;
    }

    const targetBlock = blocks.get(connection.target.block_id);
    if (!targetBlock) {
      issues.push({
        code: "missing-block",
        graphId: graph.id,
        connectionId: connection.id,
        blockId: connection.target.block_id,
        message: jaJP.errors.validation.missingTargetBlock
          .replace("{connectionId}", connection.id)
          .replace("{blockId}", connection.target.block_id),
      });
      continue;
    }

    const source = findPort(blocks, connection.source);
    if (!source) {
      issues.push({
        code: "missing-port",
        graphId: graph.id,
        connectionId: connection.id,
        blockId: connection.source.block_id,
        portId: connection.source.port_id,
        message: jaJP.errors.validation.missingSourcePort
          .replace("{connectionId}", connection.id)
          .replace("{blockId}", connection.source.block_id)
          .replace("{portId}", connection.source.port_id),
      });
      continue;
    }

    const target = findPort(blocks, connection.target);
    if (!target) {
      issues.push({
        code: "missing-port",
        graphId: graph.id,
        connectionId: connection.id,
        blockId: connection.target.block_id,
        portId: connection.target.port_id,
        message: jaJP.errors.validation.missingTargetPort
          .replace("{connectionId}", connection.id)
          .replace("{blockId}", connection.target.block_id)
          .replace("{portId}", connection.target.port_id),
      });
      continue;
    }

    if (source.direction !== "out" || target.direction !== "in") {
      issues.push({
        code: "invalid-connection-direction",
        graphId: graph.id,
        connectionId: connection.id,
        message: jaJP.errors.validation.directionMismatch
          .replace("{connectionId}", connection.id)
          .replace("{sourceDirection}", source.direction)
          .replace("{targetDirection}", target.direction),
      });
    }

    if (source.channel !== target.channel) {
      issues.push({
        code: "channel-mismatch",
        graphId: graph.id,
        connectionId: connection.id,
        message: jaJP.errors.validation.channelMismatch
          .replace("{connectionId}", connection.id)
          .replace("{sourceChannel}", source.channel)
          .replace("{targetChannel}", target.channel),
      });
    }
  }

  return issues;
}
