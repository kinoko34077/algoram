import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const canvas = await readFile(
  new URL("../src/GraphCanvas.tsx", import.meta.url),
  "utf8",
);
const app = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");

assert.match(canvas, /!pointerDragActive\.current/);
assert.match(canvas, /pointerDragActive\.current = true/);
assert.match(canvas, /pointerDragActive\.current = false/);
assert.match(
  canvas,
  /onNodeDragStop=\{\(_, node\) => \{[\s\S]*?commitNodePosition\(node\);[\s\S]*?node-drag-stop[\s\S]*?\}\}/,
);
assert.match(canvas, /onlyRenderVisibleElements/);
assert.match(canvas, /autoPanOnNodeFocus=\{false\}/);
assert.equal(
  /\n\s*fitView\s*\n\s*colorMode=/.test(canvas),
  false,
  "ReactFlow must not own implicit fitView after the explicit initial/navigation fitView boundaries",
);
assert.match(
  canvas,
  /if \(authoringStatus\) \{[\s\S]*?setInteractionStatus\(null\);[\s\S]*?\}/,
);
assert.match(canvas, /!authoringStatus && interactionStatus/);
assert.equal(
  canvas.includes("setInteractionStatus(\`Added \${template.label}.\`);"),
  false,
  "palette Add must not duplicate App-level canonical authoring feedback",
);
assert.match(canvas, /focusRequest\.blockId/);
assert.equal(
  canvas.includes("selectedBlockId }],\n        padding: 0.55"),
  false,
  "ordinary selection must not drive fitView",
);

assert.match(app, /interface GraphFocusRequest/);
assert.match(app, /setFocusRequest/);
assert.match(app, /focusRequest\?\.graphId === currentGraphId/);

console.log(
  JSON.stringify({
    kind: "smooth-interaction-guard",
    pointer_drag_app_commits: "single-boundary",
    explicit_navigation_focus: "separate",
    visible_element_rendering: "enabled",
    implicit_fit_view: "disabled",
    canonical_feedback_precedence: "single-current-message",
  }),
);
