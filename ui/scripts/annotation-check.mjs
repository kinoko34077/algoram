import assert from "node:assert/strict";
import {
  getBlockAnnotation,
  removeBlockAnnotation,
  setBlockAnnotation,
} from "../src/annotations.ts";

const empty = {};
assert.equal(getBlockAnnotation(empty, "block:a"), "");

const added = setBlockAnnotation(empty, "block:a", "first note");
assert.notStrictEqual(added, empty);
assert.deepEqual(empty, {});
assert.equal(getBlockAnnotation(added, "block:a"), "first note");

const withOther = setBlockAnnotation(added, "block:b", "other");
const updated = setBlockAnnotation(withOther, "block:a", "updated note");
assert.notStrictEqual(updated, withOther);
assert.equal(getBlockAnnotation(updated, "block:a"), "updated note");
assert.equal(getBlockAnnotation(updated, "block:b"), "other");
assert.equal(getBlockAnnotation(withOther, "block:a"), "first note");

const unchanged = setBlockAnnotation(updated, "block:a", "updated note");
assert.strictEqual(unchanged, updated);

const removed = removeBlockAnnotation(updated, "block:a");
assert.notStrictEqual(removed, updated);
assert.equal(getBlockAnnotation(removed, "block:a"), "");
assert.equal(getBlockAnnotation(removed, "block:b"), "other");
assert.equal(getBlockAnnotation(updated, "block:a"), "updated note");

const absentRemoval = removeBlockAnnotation(removed, "block:missing");
assert.strictEqual(absentRemoval, removed);

const emptied = setBlockAnnotation(updated, "block:a", "");
assert.equal(getBlockAnnotation(emptied, "block:a"), "");
assert.equal(getBlockAnnotation(emptied, "block:b"), "other");

console.log(
  JSON.stringify({
    kind: "annotation-check",
    add: "pass",
    update: "pass",
    remove: "pass",
    immutable: true,
  }),
);
