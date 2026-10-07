import assert from "node:assert/strict";
import {
  EDITOR_SESSION_SCHEMA_VERSION,
  createPersistedEditorSession,
  restoreEditorSession,
} from "../src/documentPersistence.ts";

const root = {
  schema_version: "algoram.graph/0.1",
  id: "graph:root",
  label: "Root",
  blocks: [
    {
      id: "block:child",
      label: "Child",
      internal_graph_ref: "graph:child",
      ports: [],
    },
    {
      id: "block:plain",
      label: "Plain",
      ports: [],
    },
  ],
  connections: [
    {
      id: "connection:root",
      source: { block_id: "block:child", port_id: "out" },
      target: { block_id: "block:plain", port_id: "in" },
    },
  ],
};

const child = {
  schema_version: "algoram.graph/0.1",
  id: "graph:child",
  label: "Child",
  blocks: [{ id: "block:inner", label: "Inner", ports: [] }],
  connections: [],
};

const graphs = {
  [root.id]: root,
  [child.id]: child,
};

const session = createPersistedEditorSession(
  root.id,
  graphs,
  [root.id, child.id, "graph:missing"],
  {
    [root.id]: {
      positions: {
        "block:child": { x: 10, y: 20 },
        "block:stale": { x: 99, y: 99 },
      },
      draftLinks: [
        {
          id: "draft:transient",
          sourceBlockId: "block:child",
          sourcePortId: "out",
          targetBlockId: "block:plain",
          targetPortId: "in",
        },
      ],
      viewport: { x: 100, y: -40, zoom: 1.25 },
    },
  },
  {
    [root.id]: "block:child",
    [child.id]: "block:stale",
  },
  {
    [root.id]: "connection:root",
    [child.id]: "connection:stale",
  },
  false,
);

assert.equal(session.schema_version, EDITOR_SESSION_SCHEMA_VERSION);
assert.deepEqual(session.graph_path, [root.id, child.id]);
assert.deepEqual(session.presentations[root.id].positions, {
  "block:child": { x: 10, y: 20 },
});
assert.deepEqual(session.presentations[root.id].viewport, {
  x: 100,
  y: -40,
  zoom: 1.25,
});
assert.equal("draftLinks" in session.presentations[root.id], false);
assert.equal(session.selected_block_ids[root.id], "block:child");
assert.equal(session.selected_block_ids[child.id], undefined);
assert.equal(session.selected_connection_ids[root.id], "connection:root");
assert.equal(session.selected_connection_ids[child.id], undefined);
assert.equal(session.inspector_open, false);

const restored = restoreEditorSession(root, graphs, session);
assert.deepEqual(restored.graphPath, [root.id, child.id]);
assert.deepEqual(restored.presentations[root.id], {
  positions: {
    "block:child": { x: 10, y: 20 },
  },
  draftLinks: [],
  viewport: { x: 100, y: -40, zoom: 1.25 },
});
assert.equal(restored.selectedBlockIds[root.id], "block:child");
assert.equal(restored.selectedConnectionIds[root.id], "connection:root");
assert.equal(restored.inspectorOpen, false);

const staleSession = {
  ...session,
  root_graph_id: root.id,
  graph_path: [root.id, "graph:not-a-child"],
  presentations: {
    [root.id]: {
      positions: {
        "block:missing": { x: 5, y: 5 },
        "block:plain": { x: Number.NaN, y: 3 },
      },
      viewport: { x: 0, y: 0, zoom: 0 },
    },
  },
  selected_block_ids: { [root.id]: "block:missing" },
  selected_connection_ids: { [root.id]: "connection:missing" },
};

const sanitized = restoreEditorSession(root, graphs, staleSession);
assert.deepEqual(sanitized.graphPath, [root.id]);
assert.deepEqual(sanitized.presentations[root.id], {
  positions: {},
  draftLinks: [],
});
assert.deepEqual(sanitized.selectedBlockIds, {});
assert.deepEqual(sanitized.selectedConnectionIds, {});

const mismatched = restoreEditorSession(root, graphs, {
  ...session,
  root_graph_id: "graph:other",
});
assert.deepEqual(mismatched, {
  graphPath: [root.id],
  presentations: {},
  selectedBlockIds: {},
  selectedConnectionIds: {},
  inspectorOpen: true,
});

console.log(
  JSON.stringify({
    kind: "document-persistence-check",
    canonical_and_session_separate: true,
    drafts_not_persisted: true,
    viewport_round_trip: true,
    stale_context_sanitized: true,
  }),
);
