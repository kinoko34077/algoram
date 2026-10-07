import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

async function source(path) {
  return readFile(new URL(path, import.meta.url), "utf8");
}

const [app, canvas, draft, search, annotation, authoring, styles] = await Promise.all([
  source("../src/App.tsx"),
  source("../src/GraphCanvas.tsx"),
  source("../src/DraftLinkPanel.tsx"),
  source("../src/SearchPanel.tsx"),
  source("../src/AnnotationPanel.tsx"),
  source("../src/GraphAuthoringPanel.tsx"),
  source("../src/styles.css"),
]);

assert.match(styles, /--inspector-width:\s*328px/);
assert.equal(styles.includes("30vw"), false);
assert.match(styles, /:focus-visible/);
assert.match(styles, /\.algoram-handle\s*\{[\s\S]*?width:\s*24px/);
assert.match(styles, /\.search-result-list/);
assert.equal(styles.includes("repeat(auto-fit"), false);
assert.match(styles, /--color-accent:/);
assert.match(styles, /--control-height:/);

assert.match(app, /type PresentationByGraph/);
assert.match(app, /presentationByGraph/);
assert.match(app, /type SelectionByGraph/);
assert.match(app, /inspectorOpen/);
assert.match(app, /aria-expanded=\{inspectorOpen\}/);
assert.match(app, /const selectBlock = useCallback/);
assert.match(app, /if \(previous === blockId\)/);
assert.match(app, /onPresentationChange=\{updateCurrentPresentation\}/);

assert.match(canvas, /nodesFocusable/);
assert.match(canvas, /edgesFocusable=\{false\}/);
assert.match(canvas, /disableKeyboardA11y=\{false\}/);
assert.match(canvas, /const ariaLabelConfig = \{/);
assert.match(canvas, /ariaLabelConfig=\{ariaLabelConfig\}/);
assert.match(canvas, /"node\.a11yDescription\.default": nodeA11yDescription/);
assert.match(
  canvas,
  /"node\.a11yDescription\.keyboardDisabled": nodeA11yDescription/,
);
assert.match(canvas, /Use the arrow keys to move a selected node/);
assert.equal(canvas.includes("Press delete"), false);
assert.match(canvas, /connectOnClick/);
assert.match(canvas, /change\.type === "select"/);
assert.match(canvas, /onSelectBlock\(selected\.id\)/);
assert.equal(canvas.includes("onSelectionChange="), false);
assert.match(canvas, /isValidConnection=\{isValidConnection\}/);
assert.match(canvas, /aria-live="polite"/);
assert.match(canvas, /Reset layout/);
assert.match(canvas, /Clear drafts/);

assert.match(draft, /<form/);
assert.match(draft, /<label>/);
assert.match(draft, /<select/);
assert.match(draft, /Keyboard alternative to handle dragging/);
assert.match(draft, /role="status"/);

assert.match(search, /event\.key === "Escape"/);
assert.match(search, /<ul className="search-result-list">/);
assert.match(search, /role="status"/);

assert.match(annotation, /className="danger-action"/);

assert.match(authoring, /<label>/);
assert.match(authoring, /event\.key === "Escape"/);
assert.match(authoring, /aria-invalid=\{displayedIssues\.length > 0\}/);
assert.match(authoring, /role="alert"/);
assert.match(authoring, /Read only/);
assert.match(app, /textEditorOwnsUndo/);
assert.match(app, /event\.metaKey \|\| event\.ctrlKey/);
assert.match(app, /undoCurrentGraph/);
assert.match(app, /redoCurrentGraph/);

assert.match(authoring, /aria-label="Graph properties"/);
assert.match(authoring, /Editable/);
assert.match(authoring, /Read only/);
assert.match(authoring, /Modified/);
assert.match(authoring, /Clean/);
assert.match(authoring, /event\.key === "Escape"/);
assert.match(authoring, /<label>/);
assert.match(authoring, /aria-describedby=\{/);
assert.match(authoring, /graph-label-help graph-validation/);
assert.match(app, /aria-label="Graph history"/);
assert.match(app, />\s*Undo\s*</);
assert.match(app, />\s*Redo\s*</);
assert.match(app, /textEditorOwnsUndo/);

console.log(
  JSON.stringify({
    kind: "ui-protocol-check",
    design_tokens: "pass",
    bounded_inspector: "pass",
    compact_search: "pass",
    focus_visible: "pass",
    drag_alternative_surface: "pass",
    keyboard_recovery_hooks: "pass",
    destructive_action_separation: "pass",
    canonical_authoring_form: "pass",
    native_text_undo_boundary: "pass",
    canonical_authoring_status: "pass",
    history_recovery_surface: "pass",
  }),
);
