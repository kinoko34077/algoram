import assert from "node:assert/strict";
import {
  collectReusableBlockTemplates,
  instantiateReusableBlock,
  nextPlacedBlockId,
  removeBlock,
} from "../src/blockAuthoring.ts";
import { applyGraph, createGraphHistory, undoGraph } from "../src/authoring.ts";

const template = {
  id: "block:template",
  label: "Alpha",
  definition_ref: "definition:alpha",
  internal_graph_ref: "graph:internal:alpha",
  ports: [
    { id: "in", direction: "in", channel: "data" },
    { id: "out", direction: "out", channel: "data" },
  ],
  extensions: { composite: { definition_id: "definition:alpha" } },
};
const target = {
  id: "block:target",
  label: "Target",
  ports: [{ id: "in", direction: "in", channel: "data" }],
};
const graph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:root",
  blocks: [template, target],
  connections: [],
};
const bundle = {
  rootGraphId: graph.id,
  editableGraphIds: [graph.id],
  graphs: {
    [graph.id]: graph,
    "graph:duplicate": {
      ...graph,
      id: "graph:duplicate",
      blocks: [{ ...template, id: "block:duplicate" }],
    },
  },
  sources: {},
};

const before = structuredClone(bundle);
const templates = collectReusableBlockTemplates(bundle);
assert.equal(templates.length, 1);
assert.equal(templates[0].definitionRef, "definition:alpha");
assert.deepEqual(bundle, before);

const placed = instantiateReusableBlock(graph, templates[0]);
assert.equal(placed.block.id, "block:placed:alpha:1");
assert.equal(nextPlacedBlockId(placed.graph, "definition:alpha"), "block:placed:alpha:2");
assert.equal(placed.block.definition_ref, template.definition_ref);
assert.equal(placed.block.internal_graph_ref, template.internal_graph_ref);
assert.deepEqual(placed.block.ports, template.ports);
assert.notStrictEqual(placed.block.ports, template.ports);
assert.deepEqual(bundle, before);

let history = createGraphHistory(graph);
history = applyGraph(history, placed.graph);
assert.equal(history.past.length, 1);
assert.equal(undoGraph(history).present.graph.blocks.length, 2);

const removed = removeBlock(placed.graph, placed.block.id);
assert.equal(removed.ok, true);
if (removed.ok) {
  const removalHistory = applyGraph(createGraphHistory(placed.graph), removed.graph);
  assert.equal(undoGraph(removalHistory).present.graph.blocks.length, 3);
}

const connected = {
  ...placed.graph,
  connections: [{
    id: "connection:incident",
    source: { block_id: placed.block.id, port_id: "out" },
    target: { block_id: target.id, port_id: "in" },
  }],
};
const rejected = removeBlock(connected, placed.block.id);
assert.equal(rejected.ok, false);
if (!rejected.ok) {
  assert.match(rejected.reason, /connection:incident/i);
  assert.strictEqual(rejected.graph, connected);
}

console.log(JSON.stringify({
  kind: "block-authoring-check",
  templates: "derived",
  ids: "deterministic",
  add_remove_history: "pass",
  incident_remove: "rejected",
}));
