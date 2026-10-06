import assert from "node:assert/strict";
import {
  EMPTY_GRAPH_PRESENTATION,
  addDraftLink,
  clearDraftLinks,
  makeDraftLink,
  resetNodePositions,
  setNodePosition,
  validateDraftLink,
} from "../src/presentation.ts";

const graph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:test",
  blocks: [
    {
      id: "source",
      label: "Source",
      ports: [
        { id: "out:data", direction: "out", channel: "data" },
        { id: "out:flow", direction: "out", channel: "flow" },
      ],
    },
    {
      id: "target",
      label: "Target",
      ports: [
        { id: "in:data", direction: "in", channel: "data" },
        { id: "in:flow", direction: "in", channel: "flow" },
      ],
    },
  ],
  connections: [],
};

const graphBefore = structuredClone(graph);
const moved = setNodePosition(EMPTY_GRAPH_PRESENTATION, "source", {
  x: 120,
  y: 40,
});
assert.notStrictEqual(moved, EMPTY_GRAPH_PRESENTATION);
assert.deepEqual(moved.positions.source, { x: 120, y: 40 });
assert.deepEqual(EMPTY_GRAPH_PRESENTATION.positions, {});

const link = makeDraftLink("source", "out:data", "target", "in:data");
assert.equal(validateDraftLink(graph, moved, link), null);

const linked = addDraftLink(graph, moved, link);
assert.equal(linked.draftLinks.length, 1);
assert.notStrictEqual(linked, moved);
assert.match(validateDraftLink(graph, linked, link) ?? "", /already exists/i);

const wrongChannel = makeDraftLink(
  "source",
  "out:data",
  "target",
  "in:flow",
);
assert.match(
  validateDraftLink(graph, linked, wrongChannel) ?? "",
  /same flow\/data channel/i,
);

const cleared = clearDraftLinks(linked);
assert.equal(cleared.draftLinks.length, 0);
assert.deepEqual(cleared.positions.source, { x: 120, y: 40 });

const reset = resetNodePositions(cleared);
assert.deepEqual(reset.positions, {});
assert.equal(reset.draftLinks.length, 0);

assert.deepEqual(graph, graphBefore, "presentation state must not mutate Graph");

console.log(
  JSON.stringify({
    kind: "presentation-check",
    graph_immutable: true,
    position_state_immutable: true,
    draft_validation: "pass",
    draft_recovery: "pass",
  }),
);
