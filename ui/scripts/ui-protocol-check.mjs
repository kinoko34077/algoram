import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

async function source(path) {
  return readFile(new URL(path, import.meta.url), "utf8");
}

const [app, canvas, palette, draft, search, annotation, authoring, execution, recovery, bridge, traceDetail, traceUnmapped, traceProjection, styles, traceStyles] = await Promise.all([
  source("../src/App.tsx"),
  source("../src/GraphCanvas.tsx"),
  source("../src/BlockPalette.tsx"),
  source("../src/DraftLinkPanel.tsx"),
  source("../src/SearchPanel.tsx"),
  source("../src/AnnotationPanel.tsx"),
  source("../src/GraphAuthoringPanel.tsx"),
  source("../src/ExecutionPanel.tsx"),
  source("../src/RecoveryPanel.tsx"),
  source("../src/runtimeBridge.ts"),
  source("../src/TraceDetailPanel.tsx"),
  source("../src/TraceUnmappedPanel.tsx"),
  source("../src/traceProjection.ts"),
  source("../src/styles.css"),
  source("../src/trace.css"),
]);

assert.match(styles, /--inspector-width:\s*328px/);
assert.equal(styles.includes("30vw"), false);
assert.match(styles, /:focus-visible/);
assert.match(styles, /\.algoram-handle\s*\{[\s\S]*?width:\s*24px/);
assert.match(styles, /\.search-result-list/);
assert.equal(styles.includes("repeat(auto-fit"), false);
assert.match(styles, /--color-accent:/);
assert.match(styles, /--control-height:/);
assert.match(styles, /--palette-width:\s*216px/);
assert.match(styles, /\.canvas-work-area\.palette-open/);

assert.match(app, /type PresentationByGraph/);
assert.match(app, /presentationByGraph/);
assert.match(app, /type SelectionByGraph/);
assert.match(app, /inspectorOpen/);
assert.match(app, /aria-expanded=\{inspectorOpen\}/);
assert.match(app, /const selectBlock = useCallback/);
assert.match(app, /if \(previous === blockId\)/);
assert.match(app, /onPresentationChange=\{updateCurrentPresentation\}/);

assert.match(canvas, /nodesFocusable/);
assert.match(canvas, /edgesFocusable/);
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
assert.match(canvas, /aria-controls="block-palette"/);
assert.match(canvas, /screenToFlowPosition/);
assert.match(canvas, /editable && paletteOpen/);
assert.match(canvas, /nodesConnectable=\{editable\}/);
assert.match(canvas, /onEdgeClick=/);
assert.match(canvas, /selectedConnectionId/);
assert.match(canvas, /onAddConnection/);
assert.match(canvas, /traceProjection/);
assert.match(canvas, /Observed \{Object\.keys\(traceProjection\.byBlockId\)\.length\} Blocks/);

assert.match(palette, /aria-label="Reusable Block palette"/);
assert.match(palette, /type="search"/);
assert.match(palette, /aria-pressed=\{selectedRow\}/);
assert.match(palette, /event\.key === "Escape"/);
assert.match(palette, />\s*Add Block\s*</);

assert.match(draft, /<form/);
assert.match(draft, /<label>/);
assert.match(draft, /<select/);
assert.match(draft, /Keyboard alternative to handle dragging/);
assert.match(draft, /role="status"/);
assert.match(draft, /mode === "canonical"/);
assert.match(draft, /Create Connection/);
assert.match(draft, /presentation only/);

assert.match(search, /event\.key === "Escape"/);
assert.match(search, /<ul className="search-result-list">/);
assert.match(search, /role="status"/);

assert.match(annotation, /className="danger-action"/);

assert.match(authoring, /<label>/);
assert.match(authoring, /event\.key === "Escape"/);
assert.match(authoring, /aria-invalid=\{displayedIssues\.length > 0\}/);
assert.match(authoring, /role="alert"/);
assert.match(authoring, /Read only/);
assert.match(app, /textEditorOwnsKeys/);
assert.match(app, /event\.metaKey \|\| event\.ctrlKey/);
assert.match(app, /undoCurrentGraph/);
assert.match(app, /redoCurrentGraph/);
assert.match(app, /event\.key === "Delete"/);
assert.match(app, /event\.key === "Backspace"/);
assert.match(app, /className="danger-action"/);
assert.match(app, />\s*Remove Block\s*</);
assert.match(app, />\s*Remove Connection\s*</);
assert.match(app, /selectedConnectionId/);
assert.match(app, /addCanonicalConnection/);
assert.match(app, /validateCanonicalLink/);

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
assert.match(app, /textEditorOwnsKeys/);

assert.match(styles, /\.primary-action/);
assert.match(styles, /\.execution-panel/);
assert.match(app, /"Plan"/);
assert.match(app, />\s*Run\s*</);
assert.match(app, /className="primary-action"/);
assert.match(app, /planCurrentGraph/);
assert.match(app, /runCurrentGraph/);
assert.match(app, /currentValidationIssues\.length > 0/);
assert.match(execution, /aria-label="Runtime execution"/);
assert.match(execution, /type="password"/);
assert.match(execution, /ambient host-process authority/);
assert.match(execution, /disabled=\{phase === "running"\}/);
assert.match(execution, /type="checkbox"/);
assert.match(execution, /role="alert"/);
assert.match(execution, /groupRequirements/);
assert.match(execution, /event\.isComposing/);
assert.match(execution, /Retry plan/);
assert.match(execution, /Working directory:/);
assert.match(app, /executionPhase === "ready" && executionPreview/);
assert.match(app, /new AbortController\(\)/);
assert.match(app, /controller\.signal/);
assert.match(app, /samePlan/);
assert.match(bridge, /isLoopbackHost/);
assert.match(bridge, /Authorization:/);
assert.match(bridge, /createRuntimeBridgeClient/);
assert.match(bridge, /fetchImpl/);
assert.match(bridge, /signal\?: AbortSignal/);
assert.equal(bridge.includes("child_process"), false);
assert.equal(bridge.includes("node:"), false);
assert.match(bridge, /\/v1\/recovery\/options/);
assert.match(bridge, /\/v1\/recovery\/plan/);
assert.match(bridge, /\/v1\/recovery\/run/);

assert.match(recovery, /aria-label="Manual recovery"/);
assert.match(recovery, /type="radio"/);
assert.match(recovery, /Apply &amp; re-plan/);
assert.match(recovery, /does nothing until Apply/);
assert.match(recovery, /No local fallback will be used/);
assert.match(recovery, /Marketplace catalog is not connected/);
assert.match(recovery, /hasExecutableRecoverySelection/);
assert.match(app, /requestRecoveryOptions/);
assert.match(app, /recoveryPlanGraph/);
assert.match(app, /recoveryRunGraph/);
assert.match(app, /activeRecoverySelections/);
assert.match(app, /discoverRecoveryOptions/);
assert.match(styles, /\.recovery-panel/);
assert.match(styles, /\.recovery-choice/);

assert.match(app, /projectExecutionTrace/);
assert.match(app, /traceProjection=\{currentTraceProjection\}/);
assert.match(app, /TraceDetailPanel/);
assert.match(app, /TraceUnmappedPanel/);
assert.match(canvas, /traceProjection/);
assert.match(canvas, /observedStatusLabel/);
assert.match(canvas, /Observed \{Object\.keys\(traceProjection\.byBlockId\)\.length\} Blocks/);
assert.match(traceDetail, /Observed run/);
assert.match(traceDetail, /Observed route/);
assert.match(traceDetail, /stderr/);
assert.match(traceDetail, /Route IDs are runtime evidence, not/);
assert.match(traceUnmapped, /unmapped trace origin/);
assert.match(traceUnmapped, /Unknown origin IDs are not attached/);
assert.match(traceProjection, /trace\.reference_graph_id !== graph\.id/);
assert.match(traceProjection, /origin_block_ids/);
assert.match(traceProjection, /unmappedOrigins/);
assert.equal(traceProjection.includes("runtime_ref"), false);
assert.match(traceStyles, /\.observed-status\.succeeded/);
assert.match(traceStyles, /\.observed-status\.failed/);
assert.match(traceStyles, /\.trace-detail-panel/);
assert.match(traceStyles, /overflow-wrap:\s*anywhere/);

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
    reusable_block_palette: "pass",
    keyboard_block_remove: "pass",
    canonical_connection_authoring: "pass",
    canonical_connection_selection: "pass",
    guarded_execution_panel: "pass",
    unique_explicit_grants: "pass",
    execution_retry_replans: "pass",
    execution_plan_cancel: "pass",
    observed_trace_overlay: "pass",
    trace_evidence_boundary: "pass",
    trace_text_symbol_semantics: "pass",
    unmapped_trace_evidence: "pass",
    manual_recovery_panel: "pass",
    explicit_recovery_apply: "pass",
    remote_runtime_no_fallback_boundary: "pass",
  }),
);
