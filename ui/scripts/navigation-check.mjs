import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  buildNavigationIndex,
  resolveGraphPath,
  searchNavigation,
} from "../src/navigation.ts";

function graph(id, label, blocks, sourceArtifacts = []) {
  return {
    schema_version: "algoram.graph/0.1",
    id,
    label,
    blocks,
    connections: [],
    source_artifacts: sourceArtifacts,
  };
}

function block(id, label, internalGraphRef, sourceAnchor) {
  return {
    id,
    label,
    ...(internalGraphRef ? { internal_graph_ref: internalGraphRef } : {}),
    ...(sourceAnchor ? { source_anchor: sourceAnchor } : {}),
  };
}

const tsArtifact = {
  id: "artifact:common",
  origin: "src/common.ts",
  language: "typescript",
};
const pyArtifact = {
  id: "artifact:python",
  origin: "src/other.py",
  language: "python",
};

const bundle = {
  rootGraphId: "graph:root",
  graphs: {
    "graph:root": graph("graph:root", "Repository", [
      block("block:branch-a", "Module A", "graph:a"),
      block("block:branch-b", "Module B", "graph:b"),
    ]),
    "graph:a": graph("graph:a", "Module A", [
      block("block:a-common", "Shared from A", "graph:common"),
    ]),
    "graph:b": graph("graph:b", "Module B", [
      block("block:b-common", "Shared from B", "graph:common"),
      block(
        "block:python-needle",
        "Needle helper",
        undefined,
        {
          artifact_id: "artifact:python",
          start_byte: 0,
          end_byte: 6,
          semantic_key: "python#needle",
        },
      ),
    ], [pyArtifact]),
    "graph:common": graph("graph:common", "Shared", [
      block(
        "block:deep-target",
        "Needle target",
        undefined,
        {
          artifact_id: "artifact:common",
          start_byte: 0,
          end_byte: 8,
          semantic_key: "common#deep",
        },
      ),
      block("block:cycle", "Cycle to A", "graph:a"),
    ], [tsArtifact]),
    "graph:orphan": graph("graph:orphan", "Orphan", [
      block("block:orphan-target", "Orphan target"),
    ]),
  },
  sources: {
    "artifact:common": {
      artifact: tsArtifact,
      text: "export const target = 1;",
    },
    "artifact:python": {
      artifact: pyArtifact,
      text: "print('needle')",
    },
  },
};

const index = buildNavigationIndex(bundle);

assert.equal(index.records.length, 8);

const byLabel = searchNavigation(index, "needle");
assert.deepEqual(
  byLabel.map((record) => record.blockId),
  ["block:python-needle", "block:deep-target"],
);

const bySource = searchNavigation(index, "src/common.ts");
assert.deepEqual(
  bySource.map((record) => record.blockId),
  ["block:deep-target"],
);

const bySemanticKey = searchNavigation(index, "common#deep");
assert.deepEqual(
  bySemanticKey.map((record) => record.blockId),
  ["block:deep-target"],
);

const filtered = searchNavigation(index, "needle", {
  language: "TypeScript",
});
assert.deepEqual(
  filtered.map((record) => record.blockId),
  ["block:deep-target"],
);

const commonPath = resolveGraphPath(index, "graph:common");
assert.deepEqual(
  commonPath?.map((entry) => [entry.graphId, entry.viaBlockId]),
  [
    ["graph:root", undefined],
    ["graph:a", "block:branch-a"],
    ["graph:common", "block:a-common"],
  ],
);

assert.equal(resolveGraphPath(index, "graph:orphan"), null);
assert.equal(resolveGraphPath(index, "graph:missing"), null);

const source = await readFile(
  new URL("../src/navigation.ts", import.meta.url),
  "utf8",
);
for (const forbidden of [
  "react",
  "@xyflow/react",
  "elkjs",
  "GraphCanvas",
]) {
  assert.equal(
    source.includes(forbidden),
    false,
    `navigation.ts must not depend on ${forbidden}`,
  );
}

console.log(
  JSON.stringify({
    kind: "navigation-check",
    indexed_records: index.records.length,
    containment_edges: index.containmentEdges.length,
    deterministic_path: commonPath?.map((entry) => entry.graphId),
  }),
);
