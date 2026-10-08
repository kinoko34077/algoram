import assert from "node:assert/strict";
import { getBlockGeometry, portHandleTop, projectBlockDisplay } from "../src/blockDisplay.ts";
import { demoJa } from "../src/locales/ja-JP/demo.ts";

const one = (id, label, ports = []) => ({ id, label, ports });
const out = (id) => ({ id, direction: "out", channel: "data", contract: "int32" });
const input = (id) => ({ id, direction: "in", channel: "data", contract: "int32" });

const tiny = one("canonical:unchanged", "×2", [input("v"), out("res")]);
const complex = one("complex", "条件で分ける", [input("src"), out("yes"), out("no"), out("maybe")]);
const unknown = one("unknown", "CustomCalc<Any>");
const known = one("source-backed", demoJa.graphPythonC, [input("value"), out("result")]);
const before = JSON.stringify([tiny, complex, unknown, known]);

const a = projectBlockDisplay(tiny);
const b = projectBlockDisplay(complex);
const c = projectBlockDisplay(unknown);
const d = projectBlockDisplay(known);

assert.equal(a.shortLabel, "×2");
assert.equal(c.shortLabel, "CustomCalc<Any>", "unknown technical labels are not guessed");
assert.equal(d.shortLabel, "Py→C ×2");
assert.equal(d.fullLabel, demoJa.graphPythonC);
assert.equal(d.gloss, demoJa.graphPythonC);
assert.ok(a.geometry.width < b.geometry.width);
assert.ok(a.geometry.width < 244);
assert.ok(b.geometry.height > a.geometry.height, "more ports demand more vertical space");
assert.deepEqual(a.geometry, getBlockGeometry(tiny));
assert.ok(portHandleTop(0, 3, b.geometry.height) < portHandleTop(1, 3, b.geometry.height));
assert.ok(portHandleTop(2, 3, b.geometry.height) <= b.geometry.height - 12);
assert.throws(() => portHandleTop(0, 0, 60), RangeError);
assert.equal(JSON.stringify([tiny, complex, unknown, known]), before, "display projection must not mutate Graph data");
console.log("PASS block display projection: compact/intrinsic/multiport/immutable/unknown");
