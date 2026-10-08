import assert from "node:assert/strict";
import { GLOSS_HOLD_MS, GLOSS_DRAG_SLOP_PX, hasMovedBeyondGlossSlop } from "../src/glossGesture.ts";
assert.ok(GLOSS_HOLD_MS > 300 && GLOSS_HOLD_MS < 900);
assert.ok(GLOSS_DRAG_SLOP_PX >= 6);
assert.equal(hasMovedBeyondGlossSlop(0,0,3,4),false);
assert.equal(hasMovedBeyondGlossSlop(0,0,8,0),false);
assert.equal(hasMovedBeyondGlossSlop(0,0,9,0),true);
assert.equal(hasMovedBeyondGlossSlop(200,100,200,109),true);
console.log("PASS gesture hold threshold and draggable movement cancellation");
