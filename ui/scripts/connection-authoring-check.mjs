import assert from "node:assert/strict";
import {
  createConnection,
  nextConnectionId,
  removeConnection,
  validateConnectionCandidate,
} from "../src/connectionAuthoring.ts";
import { applyGraph, createGraphHistory, redoGraph, undoGraph } from "../src/authoring.ts";
import { removeBlock } from "../src/blockAuthoring.ts";
import { jaJP } from "../src/locales/ja-JP/index.ts";

const graph = {
  schema_version: "algoram.graph/0.1",
  id: "graph:connections",
  blocks: [
    {
      id: "source",
      label: "Source",
      ports: [
        { id: "data-out", direction: "out", channel: "data" },
        { id: "flow-out", direction: "out", channel: "flow" },
        { id: "wrong-in", direction: "in", channel: "data" },
      ],
    },
    {
      id: "target",
      label: "Target",
      ports: [
        { id: "data-in", direction: "in", channel: "data" },
        { id: "flow-in", direction: "in", channel: "flow" },
        { id: "wrong-out", direction: "out", channel: "data" },
      ],
    },
  ],
  connections: [],
};

const before = structuredClone(graph);
const validCandidate = {
  sourceBlockId: "source",
  sourcePortId: "data-out",
  targetBlockId: "target",
  targetPortId: "data-in",
};

assert.equal(validateConnectionCandidate(graph, validCandidate), null);
assert.equal(nextConnectionId(graph), "connection:placed:1");

const created = createConnection(graph, validCandidate);
assert.equal(created.ok, true);
if (!created.ok) throw new Error(created.reason);
assert.equal(created.connection.id, "connection:placed:1");
assert.equal(nextConnectionId(created.graph), "connection:placed:2");
assert.deepEqual(graph, before, "create must not mutate source Graph");

let history = createGraphHistory(graph);
history = applyGraph(history, created.graph);
assert.equal(history.present.graph.connections.length, 1);
assert.equal(history.past.length, 1);
history = undoGraph(history);
assert.equal(history.present.graph.connections.length, 0);
history = redoGraph(history);
assert.equal(history.present.graph.connections.length, 1);

const wrongDirection = createConnection(graph, {
  sourceBlockId: "source",
  sourcePortId: "wrong-in",
  targetBlockId: "target",
  targetPortId: "wrong-out",
});
assert.equal(wrongDirection.ok, false);
if (!wrongDirection.ok) {
  assert.equal(
    wrongDirection.reason,
    jaJP.errors.authoring.directionMismatch
      .replace("{connectionId}", "connection:placed:1")
      .replace("{sourceDirection}", "in")
      .replace("{targetDirection}", "out"),
  );
  assert.strictEqual(wrongDirection.graph, graph);
}

const wrongChannel = createConnection(graph, {
  sourceBlockId: "source",
  sourcePortId: "flow-out",
  targetBlockId: "target",
  targetPortId: "data-in",
});
assert.equal(wrongChannel.ok, false);
if (!wrongChannel.ok) {
  assert.equal(
    wrongChannel.reason,
    jaJP.errors.authoring.channelMismatch
      .replace("{connectionId}", "connection:placed:1")
      .replace("{sourceChannel}", "flow")
      .replace("{targetChannel}", "data"),
  );
}

const missing = createConnection(graph, {
  sourceBlockId: "source",
  sourcePortId: "missing",
  targetBlockId: "target",
  targetPortId: "data-in",
});
assert.equal(missing.ok, false);
if (!missing.ok) {
  assert.equal(
    missing.reason,
    jaJP.errors.authoring.connectionMissingSourcePort
      .replace("{connectionId}", "connection:placed:1")
      .replace("{blockId}", "source")
      .replace("{portId}", "missing"),
  );
}

const blockRemoveRejected = removeBlock(created.graph, "source");
assert.equal(blockRemoveRejected.ok, false);
if (!blockRemoveRejected.ok) {
  assert.match(blockRemoveRejected.reason, /connection:placed:1/i);
}

const removed = removeConnection(created.graph, created.connection.id);
assert.equal(removed.ok, true);
if (!removed.ok) throw new Error(removed.reason);
assert.equal(removed.graph.connections.length, 0);

let removalHistory = createGraphHistory(created.graph);
removalHistory = applyGraph(removalHistory, removed.graph);
assert.equal(removalHistory.present.graph.connections.length, 0);
assert.equal(undoGraph(removalHistory).present.graph.connections.length, 1);

const blockAfterConnectionRemoval = removeBlock(removed.graph, "source");
assert.equal(blockAfterConnectionRemoval.ok, true);

console.log(
  JSON.stringify({
    kind: "connection-authoring-check",
    ids: "deterministic",
    validation: "graph-core-mirror",
    create_history: "pass",
    remove_undo: "pass",
    incident_block_remove_boundary: "pass",
  }),
);
