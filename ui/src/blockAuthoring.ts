import type {
  AlgoramBlock,
  AlgoramGraph,
  ReferenceBundle,
} from "./algoram";
import { jaJP } from "./locales/ja-JP";

export interface ReusableBlockTemplate {
  definitionRef: string;
  label: string;
  block: AlgoramBlock;
}

export type RemoveBlockResult =
  | {
      ok: true;
      graph: AlgoramGraph;
      removed: AlgoramBlock;
    }
  | {
      ok: false;
      graph: AlgoramGraph;
      reason: string;
    };

function cloneBlock(block: AlgoramBlock): AlgoramBlock {
  return structuredClone(block);
}

function blockIdSlug(definitionRef: string): string {
  const source = definitionRef.split(":").at(-1) ?? "block";
  const slug = source
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "block";
}

export function collectReusableBlockTemplates(
  bundle: ReferenceBundle,
): ReusableBlockTemplate[] {
  const byDefinition = new Map<string, ReusableBlockTemplate>();

  for (const graph of Object.values(bundle.graphs)) {
    for (const block of graph.blocks) {
      if (!block.definition_ref || byDefinition.has(block.definition_ref)) {
        continue;
      }

      byDefinition.set(block.definition_ref, {
        definitionRef: block.definition_ref,
        label: block.label,
        block: cloneBlock(block),
      });
    }
  }

  return [...byDefinition.values()].sort((left, right) => {
    const labelOrder = left.label.localeCompare(right.label);
    return labelOrder !== 0
      ? labelOrder
      : left.definitionRef.localeCompare(right.definitionRef);
  });
}

export function nextPlacedBlockId(
  graph: AlgoramGraph,
  definitionRef: string,
): string {
  const prefix = `block:placed:${blockIdSlug(definitionRef)}:`;
  const occupied = new Set(graph.blocks.map((block) => block.id));

  for (let sequence = 1; ; sequence += 1) {
    const candidate = `${prefix}${sequence}`;
    if (!occupied.has(candidate)) {
      return candidate;
    }
  }
}

export function instantiateReusableBlock(
  graph: AlgoramGraph,
  template: ReusableBlockTemplate,
): { graph: AlgoramGraph; block: AlgoramBlock } {
  const block = cloneBlock(template.block);
  block.id = nextPlacedBlockId(graph, template.definitionRef);

  const nextGraph: AlgoramGraph = {
    ...graph,
    blocks: [...graph.blocks, block],
  };

  return {
    graph: nextGraph,
    block,
  };
}

export function removeBlock(
  graph: AlgoramGraph,
  blockId: string,
): RemoveBlockResult {
  const block = graph.blocks.find((candidate) => candidate.id === blockId);
  if (!block) {
    return {
      ok: false,
      graph,
      reason: jaJP.errors.authoring.blockNoLongerExists.replace("{blockId}", blockId),
    };
  }

  const incident = graph.connections.find(
    (connection) =>
      connection.source.block_id === blockId ||
      connection.target.block_id === blockId,
  );
  if (incident) {
    return {
      ok: false,
      graph,
      reason: jaJP.errors.authoring.blockHasIncidentConnection
        .replace("{blockLabel}", block.label)
        .replace("{connectionId}", incident.id),
    };
  }

  return {
    ok: true,
    graph: {
      ...graph,
      blocks: graph.blocks.filter((candidate) => candidate.id !== blockId),
    },
    removed: cloneBlock(block),
  };
}
