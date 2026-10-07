import assert from "node:assert/strict";
import { projectExecutionTrace } from "../src/traceProjection.ts";

const graph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:trace",
  blocks: [
    { id: "block:a", label: "A", ports: [] },
    { id: "block:b", label: "B", ports: [] },
    { id: "block:c", label: "C", ports: [] },
    { id: "block:d", label: "D", ports: [] },
  ],
  connections: [],
};

function entry(step, status, origins, options = {}) {
  return {
    step_id: step,
    implementation_ref: options.implementation_ref ?? `impl:${step}`,
    status,
    origin_block_ids: origins,
    route_connector_ids: options.route_connector_ids ?? [],
    exit_code: options.exit_code,
    stdout: options.stdout ?? "",
    stderr: options.stderr ?? "",
  };
}

const mismatch = projectExecutionTrace(graph, {
  reference_graph_id: "graph:other",
  entries: [],
});
assert.equal(mismatch.ok, false);
if (!mismatch.ok) {
  assert.match(mismatch.reason, /graph:other/);
  assert.match(mismatch.reason, /graph:trace/);
}

const trace = {
  reference_graph_id: graph.id,
  entries: [
    entry("step:a1", "succeeded", ["block:a"], {
      implementation_ref: "impl:a",
      route_connector_ids: ["route:z", "route:a"],
      stdout: "A OK",
    }),
    entry("step:b1", "failed", ["block:b"], {
      implementation_ref: "impl:b",
      exit_code: 23,
      stderr: "B failed",
    }),
    entry("step:c1", "not_run", ["block:c"], {
      implementation_ref: "impl:c",
    }),
    entry("step:d1", "succeeded", ["block:d"], {
      implementation_ref: "impl:d1",
    }),
    entry("step:d2", "not_run", ["block:d"], {
      implementation_ref: "impl:d2",
    }),
    entry("step:shared", "succeeded", ["block:a", "block:b"], {
      implementation_ref: "impl:shared",
    }),
    entry("step:unknown", "failed", ["block:missing"], {
      implementation_ref: "impl:unknown",
      stderr: "unmapped",
    }),
  ],
};

const result = projectExecutionTrace(graph, trace);
assert.equal(result.ok, true);
if (!result.ok) {
  throw new Error(result.reason);
}

const projection = result.projection;
assert.equal(projection.referenceGraphId, graph.id);
assert.equal(projection.mappedEntryCount, 6);
assert.equal(projection.unmappedEntries.length, 1);
assert.equal(projection.unmappedEntries[0].step_id, "step:unknown");

assert.equal(projection.byBlockId["block:a"].status, "succeeded");
assert.deepEqual(
  projection.byBlockId["block:a"].implementationRefs,
  ["impl:a", "impl:shared"],
);
assert.deepEqual(
  projection.byBlockId["block:a"].routeConnectorIds,
  ["route:a", "route:z"],
);
assert.deepEqual(projection.byBlockId["block:a"].stdout, ["A OK"]);

assert.equal(projection.byBlockId["block:b"].status, "failed");
assert.deepEqual(projection.byBlockId["block:b"].exitCodes, [23]);
assert.deepEqual(projection.byBlockId["block:b"].stderr, ["B failed"]);

assert.equal(projection.byBlockId["block:c"].status, "not_run");
assert.equal(projection.byBlockId["block:d"].status, "mixed");
assert.deepEqual(
  projection.byBlockId["block:d"].stepIds,
  ["step:d1", "step:d2"],
);

assert.deepEqual(graph.blocks.map((block) => block.id), [
  "block:a",
  "block:b",
  "block:c",
  "block:d",
]);
assert.equal(
  "runtime_ref" in projection.byBlockId["block:a"],
  false,
  "local S5 trace projection must not invent runtime identity",
);

console.log(
  JSON.stringify({
    kind: "trace-projection-check",
    graph_identity_guard: "pass",
    exact_origin_mapping: "pass",
    aggregation: {
      succeeded: "pass",
      failed: "pass",
      not_run: "pass",
      mixed: "pass",
    },
    unmapped_evidence: "retained",
    runtime_identity: "not invented",
  }),
);
