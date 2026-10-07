import type { AlgoramGraph } from "./algoram";
import type { ExecutionTrace, TraceEntry } from "./runtimeBridge";

export type ObservedBlockStatus =
  | "succeeded"
  | "failed"
  | "not_run"
  | "mixed";

export interface BlockTraceObservation {
  blockId: string;
  status: ObservedBlockStatus;
  entries: TraceEntry[];
  stepIds: string[];
  implementationRefs: string[];
  routeConnectorIds: string[];
  exitCodes: number[];
  stderr: string[];
  stdout: string[];
}

export interface TraceProjection {
  referenceGraphId: string;
  byBlockId: Record<string, BlockTraceObservation>;
  unmappedEntries: TraceEntry[];
  mappedEntryCount: number;
}

export type TraceProjectionResult =
  | {
      ok: true;
      projection: TraceProjection;
    }
  | {
      ok: false;
      reason: string;
    };

function uniqueSorted(values: string[]): string[] {
  return [...new Set(values.filter((value) => value.length > 0))].sort();
}

function aggregateStatus(entries: TraceEntry[]): ObservedBlockStatus {
  const statuses = new Set(entries.map((entry) => entry.status));
  if (statuses.has("failed")) {
    return "failed";
  }
  if (statuses.size === 1 && statuses.has("succeeded")) {
    return "succeeded";
  }
  if (statuses.size === 1 && statuses.has("not_run")) {
    return "not_run";
  }
  return "mixed";
}

function makeObservation(
  blockId: string,
  entries: TraceEntry[],
): BlockTraceObservation {
  return {
    blockId,
    status: aggregateStatus(entries),
    entries,
    stepIds: uniqueSorted(entries.map((entry) => entry.step_id)),
    implementationRefs: uniqueSorted(
      entries.map((entry) => entry.implementation_ref),
    ),
    routeConnectorIds: uniqueSorted(
      entries.flatMap((entry) => entry.route_connector_ids ?? []),
    ),
    exitCodes: [
      ...new Set(
        entries
          .map((entry) => entry.exit_code)
          .filter((code): code is number => code !== undefined),
      ),
    ].sort((left, right) => left - right),
    stderr: entries
      .map((entry) => entry.stderr.trim())
      .filter((value) => value.length > 0),
    stdout: entries
      .map((entry) => entry.stdout.trim())
      .filter((value) => value.length > 0),
  };
}

export function projectExecutionTrace(
  graph: AlgoramGraph,
  trace: ExecutionTrace,
): TraceProjectionResult {
  if (trace.reference_graph_id !== graph.id) {
    return {
      ok: false,
      reason:
        `ExecutionTrace references Graph '${trace.reference_graph_id}', ` +
        `but the current Graph is '${graph.id}'.`,
    };
  }

  const visibleBlockIds = new Set(graph.blocks.map((block) => block.id));
  const entriesByBlock = new Map<string, TraceEntry[]>();
  const unmappedEntries: TraceEntry[] = [];
  let mappedEntryCount = 0;

  for (const entry of trace.entries) {
    const mappedBlockIds = uniqueSorted(
      entry.origin_block_ids.filter((blockId) => visibleBlockIds.has(blockId)),
    );

    if (mappedBlockIds.length === 0) {
      unmappedEntries.push(entry);
      continue;
    }

    mappedEntryCount += 1;
    for (const blockId of mappedBlockIds) {
      const entries = entriesByBlock.get(blockId) ?? [];
      entries.push(entry);
      entriesByBlock.set(blockId, entries);
    }
  }

  const byBlockId = Object.fromEntries(
    [...entriesByBlock.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([blockId, entries]) => [
        blockId,
        makeObservation(blockId, entries),
      ]),
  );

  return {
    ok: true,
    projection: {
      referenceGraphId: trace.reference_graph_id,
      byBlockId,
      unmappedEntries,
      mappedEntryCount,
    },
  };
}

export function observedStatusLabel(status: ObservedBlockStatus): string {
  switch (status) {
    case "succeeded":
      return "Succeeded";
    case "failed":
      return "Failed";
    case "not_run":
      return "Not run";
    case "mixed":
      return "Mixed";
  }
}

export function observedStatusSymbol(status: ObservedBlockStatus): string {
  switch (status) {
    case "succeeded":
      return "✓";
    case "failed":
      return "×";
    case "not_run":
      return "–";
    case "mixed":
      return "±";
  }
}
