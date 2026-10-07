import assert from "node:assert/strict";
import {
  applyGraph,
  createAuthoringHistories,
  createGraphHistory,
  graphIsDirty,
  markGraphSaved,
  redoGraph,
  setGraphLabel,
  undoGraph,
} from "../src/authoring.ts";
import { validateGraph } from "../src/graphValidation.ts";
import {
  EMPTY_GRAPH_PRESENTATION,
  setNodePosition,
} from "../src/presentation.ts";

function port(id, direction, channel) {
  return { id, direction, channel };
}

function block(id, ports = []) {
  return { id, label: id, ports };
}

const nativeGraph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:native",
  label: "Native",
  blocks: [
    block("source", [port("out", "out", "data")]),
    block("target", [port("in", "in", "data")]),
  ],
  connections: [
    {
      id: "connection:data",
      source: { block_id: "source", port_id: "out" },
      target: { block_id: "target", port_id: "in" },
    },
  ],
};

const derivedGraph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:derived",
  label: "Derived",
  blocks: [],
  connections: [],
};

const bundle = {
  rootGraphId: nativeGraph.id,
  editableGraphIds: [nativeGraph.id],
  graphs: {
    [nativeGraph.id]: nativeGraph,
    [derivedGraph.id]: derivedGraph,
  },
  sources: {},
};

const originalNative = structuredClone(nativeGraph);
const histories = createAuthoringHistories(bundle);
assert.deepEqual(Object.keys(histories), [nativeGraph.id]);
assert.notStrictEqual(histories[nativeGraph.id].present.graph, nativeGraph);
assert.deepEqual(histories[nativeGraph.id].present.graph, nativeGraph);
assert.equal(graphIsDirty(histories[nativeGraph.id]), false);

let history = histories[nativeGraph.id];
const renamed = setGraphLabel(history.present.graph, "Renamed");
history = applyGraph(history, renamed);
assert.equal(history.present.graph.label, "Renamed");
assert.equal(history.past.length, 1);
assert.equal(history.future.length, 0);
assert.equal(graphIsDirty(history), true);
assert.deepEqual(nativeGraph, originalNative, "input bundle Graph must remain unchanged");

const historyFingerprintBeforePresentation = history.present.fingerprint;
const presentationOnly = setNodePosition(
  EMPTY_GRAPH_PRESENTATION,
  "source",
  { x: 120, y: 80 },
);
assert.deepEqual(presentationOnly.positions.source, { x: 120, y: 80 });
assert.equal(
  history.present.fingerprint,
  historyFingerprintBeforePresentation,
  "presentation-only movement must not enter canonical Graph history",
);

const unchanged = applyGraph(history, setGraphLabel(history.present.graph, "Renamed"));
assert.strictEqual(unchanged, history, "semantic no-op must not create history");

history = undoGraph(history);
assert.equal(history.present.graph.label, "Native");
assert.equal(history.future.length, 1);
assert.equal(graphIsDirty(history), false);

history = redoGraph(history);
assert.equal(history.present.graph.label, "Renamed");
assert.equal(history.past.length, 1);
assert.equal(graphIsDirty(history), true);

history = markGraphSaved(history);
assert.equal(graphIsDirty(history), false);

history = applyGraph(history, setGraphLabel(history.present.graph, "Native"));
assert.equal(graphIsDirty(history), true, "returning to pre-save content is still dirty");
history = undoGraph(history);
assert.equal(graphIsDirty(history), false, "undo returns to saved fingerprint");

const blankHistory = createGraphHistory(nativeGraph);
const blankLabel = setGraphLabel(blankHistory.present.graph, "   ");
assert.equal(blankLabel.label, undefined);

let boundedHistory = createGraphHistory(nativeGraph);
for (let index = 0; index < 150; index += 1) {
  boundedHistory = applyGraph(
    boundedHistory,
    setGraphLabel(boundedHistory.present.graph, "Revision " + index),
  );
}
assert.equal(boundedHistory.past.length, 100, "history must stay bounded");

assert.deepEqual(validateGraph(nativeGraph), []);

function codes(graph) {
  return new Set(validateGraph(graph).map((issue) => issue.code));
}

assert.equal(
  codes({ ...nativeGraph, schema_version: "algoram.graph/9.9" }).has(
    "unsupported-schema-version",
  ),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    source_artifacts: [
      { id: "artifact:same", origin: "a", language: "x" },
      { id: "artifact:same", origin: "b", language: "y" },
    ],
  }).has("duplicate-source-artifact-id"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    blocks: [block("same"), block("same")],
    connections: [],
  }).has("duplicate-block-id"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    blocks: [block("p", [port("same", "out", "data"), port("same", "out", "data")])],
    connections: [],
  }).has("duplicate-port-id"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    source_artifacts: [{ id: "artifact:a", origin: "a", language: "x" }],
    blocks: [
      {
        ...block("anchored"),
        source_anchor: {
          artifact_id: "artifact:missing",
          start_byte: 20,
          end_byte: 10,
        },
      },
    ],
    connections: [],
  }).has("invalid-source-anchor-range"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    source_artifacts: [],
    blocks: [
      {
        ...block("anchored"),
        source_anchor: {
          artifact_id: "artifact:missing",
          start_byte: 0,
          end_byte: 10,
        },
      },
    ],
    connections: [],
  }).has("missing-source-artifact"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    source_artifacts: [{ id: "artifact:a", origin: "a", language: "x" }],
    blocks: [
      {
        ...block("negative-anchor"),
        source_anchor: {
          artifact_id: "artifact:a",
          start_byte: -1,
          end_byte: 10,
        },
      },
    ],
    connections: [],
  }).has("invalid-source-anchor-range"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    source_artifacts: [{ id: "artifact:a", origin: "a", language: "x" }],
    blocks: [
      {
        ...block("fractional-anchor"),
        source_anchor: {
          artifact_id: "artifact:a",
          start_byte: 0.5,
          end_byte: 10,
        },
      },
    ],
    connections: [],
  }).has("invalid-source-anchor-range"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    connections: [
      nativeGraph.connections[0],
      { ...nativeGraph.connections[0] },
    ],
  }).has("duplicate-connection-id"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    connections: [
      {
        id: "connection:missing-block",
        source: { block_id: "missing", port_id: "out" },
        target: { block_id: "target", port_id: "in" },
      },
    ],
  }).has("missing-block"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    connections: [
      {
        id: "connection:missing-port",
        source: { block_id: "source", port_id: "missing" },
        target: { block_id: "target", port_id: "in" },
      },
    ],
  }).has("missing-port"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    blocks: [
      block("source", [port("out", "in", "data")]),
      block("target", [port("in", "in", "data")]),
    ],
  }).has("invalid-connection-direction"),
  true,
);

assert.equal(
  codes({
    ...nativeGraph,
    blocks: [
      block("source", [port("out", "out", "flow")]),
      block("target", [port("in", "in", "data")]),
    ],
  }).has("channel-mismatch"),
  true,
);

console.log(
  JSON.stringify({
    kind: "canonical-authoring-check",
    editable_policy: "explicit",
    immutable_working_copy: "pass",
    history: "apply/undo/redo/mark-saved/bounded-100",
    graph_core_validation_mirror: "pass",
  }),
);
