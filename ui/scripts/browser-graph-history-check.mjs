import assert from "node:assert/strict";
import {
  makeGraphHistoryState,
  readGraphHistoryState,
} from "../src/browserGraphHistory.ts";

const available = { root: {}, child: {}, sibling: {} };
const root = [{ graphId: "root", label: "Root" }];
const child = [...root, { graphId: "child", label: "Child" }];
const encoded = makeGraphHistoryState({ otherTool: "preserved" }, "root", child);
assert.deepEqual(readGraphHistoryState(encoded, "root", available), child);
assert.equal(encoded.otherTool, "preserved");
assert.equal(readGraphHistoryState(encoded, "sibling", available), null);
assert.equal(readGraphHistoryState(null, "root", available), null);
assert.equal(
  readGraphHistoryState(makeGraphHistoryState(null, "root", [
    ...root, { graphId: "missing", label: "Not a graph" },
  ]), "root", available),
  null,
);
assert.equal(
  readGraphHistoryState(makeGraphHistoryState(null, "root", [
    { graphId: "child", label: "Wrong start" },
  ]), "root", available),
  null,
);
assert.deepEqual(readGraphHistoryState(makeGraphHistoryState(null, "root", root), "root", available), root);
assert.equal(JSON.stringify(encoded).includes("bearerToken"), false);
console.log(JSON.stringify({
  kind: "browser-graph-history-check",
  valid_path: "pass",
  stale_root: "rejected",
  unknown_graph: "rejected",
  wrong_root_entry: "rejected",
  history_extra_state: "preserved",
  graph_runtime_payload: "not serialized",
}));
