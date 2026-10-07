import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

async function source(path) {
  return readFile(new URL(path, import.meta.url), "utf8");
}

const [app, canvas, palette, draft, search, annotation, authoring, execution, recovery, bridge, traceDetail, traceUnmapped, traceProjection, fileMenu, headerOverflow, persistenceStore, persistenceModel, styles, traceStyles] = await Promise.all([
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
  source("../src/FileMenu.tsx"),
  source("../src/HeaderOverflowMenu.tsx"),
  source("../src/browserPersistence.ts"),
  source("../src/documentPersistence.ts"),
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
assert.match(canvas, /jaJP\.authoring\.canvas\.keyboardNodeHint/);
assert.equal(canvas.includes("Press delete"), false);
assert.match(canvas, /connectOnClick/);
assert.match(canvas, /change\.type === "select"/);
assert.match(canvas, /onSelectBlock\(selected\.id\)/);
assert.equal(canvas.includes("onSelectionChange="), false);
assert.match(canvas, /isValidConnection=\{isValidConnection\}/);
assert.match(canvas, /aria-live="polite"/);
assert.match(canvas, /jaJP\.authoring\.canvas\.resetLayoutAction/);
assert.match(canvas, /jaJP\.authoring\.canvas\.clearDraftsAction/);
assert.match(canvas, /aria-controls="block-palette"/);
assert.match(canvas, /screenToFlowPosition/);
assert.match(canvas, /editable && paletteOpen/);
assert.match(canvas, /nodesConnectable=\{editable\}/);
assert.match(canvas, /onEdgeClick=/);
assert.match(canvas, /selectedConnectionId/);
assert.match(canvas, /onAddConnection/);
assert.match(canvas, /traceProjection/);
assert.match(canvas, /jaJP\.authoring\.canvas\.observedBlocks/);

assert.match(palette, /jaJP\.authoring\.blockPalette\.accessibilityLabel/);
assert.match(palette, /type="search"/);
assert.match(palette, /aria-pressed=\{selectedRow\}/);
assert.match(palette, /event\.key === "Escape"/);
assert.match(palette, /jaJP\.authoring\.blockPalette\.addBlock/);

assert.match(draft, /<form/);
assert.match(draft, /<label>/);
assert.match(draft, /<select/);
assert.match(draft, /jaJP\.authoring\.draftLink\.connectionHint/);
assert.match(draft, /role="status"/);
assert.match(draft, /mode === "canonical"/);
assert.match(draft, /jaJP\.authoring\.draftLink\.createConnection/);
assert.match(draft, /jaJP\.authoring\.draftLink\.presentationOnly/);

assert.match(search, /event\.key === "Escape"/);
assert.match(search, /<ul className="search-result-list">/);
assert.match(search, /role="status"/);

assert.match(annotation, /className="danger-action"/);

assert.match(authoring, /<label>/);
assert.match(authoring, /event\.key === "Escape"/);
assert.match(authoring, /aria-invalid=\{displayedIssues\.length > 0\}/);
assert.match(authoring, /role="alert"/);
assert.match(authoring, /jaJP\.authoring\.graphProperties\.readOnly/);
assert.match(app, /textEditorOwnsKeys/);
assert.match(app, /event\.metaKey \|\| event\.ctrlKey/);
assert.match(app, /undoCurrentGraph/);
assert.match(app, /redoCurrentGraph/);
assert.match(app, /event\.key === "Delete"/);
assert.match(app, /event\.key === "Backspace"/);
assert.match(app, /className="danger-action"/);
assert.match(app, /jaJP\.app\.removeBlock/);
assert.match(app, /jaJP\.app\.removeConnection/);
assert.match(app, /selectedConnectionId/);
assert.match(app, /addCanonicalConnection/);
assert.match(app, /validateCanonicalLink/);

assert.match(authoring, /jaJP\.authoring\.graphProperties\.properties/);
assert.match(authoring, /jaJP\.authoring\.graphProperties\.editable/);
assert.match(authoring, /jaJP\.authoring\.graphProperties\.readOnly/);
assert.match(authoring, /jaJP\.authoring\.graphProperties\.modified/);
assert.match(authoring, /jaJP\.authoring\.graphProperties\.clean/);
assert.match(authoring, /event\.key === "Escape"/);
assert.match(authoring, /<label>/);
assert.match(authoring, /aria-describedby=\{/);
assert.match(authoring, /graph-label-help graph-validation/);
assert.match(app, /jaJP\.app\.graphHistory/);
assert.match(app, /jaJP\.common\.actions\.undo/);
assert.match(app, /jaJP\.common\.actions\.redo/);
assert.match(app, /textEditorOwnsKeys/);

assert.match(fileMenu, /Ctrl\/Cmd\+O/);
assert.match(fileMenu, /Ctrl\/Cmd\+S/);
assert.match(fileMenu, /type="file"/);
assert.match(fileMenu, /accept="\.algoram\.json,application\/json"/);
assert.match(fileMenu, /event\.key === "Escape"/);
assert.match(app, /readCanonicalGraphFile/);
assert.match(app, /saveLocalDocument/);
assert.match(app, /loadLastLocalDocument/);
assert.match(app, /window\.confirm/);
assert.match(app, /beforeunload/);
assert.match(app, /markGraphSaved/);
assert.match(canvas, /onMoveEnd=/);
assert.match(canvas, /setPresentationViewport/);
assert.match(canvas, /setViewport\(presentation\.viewport/);
assert.match(persistenceStore, /indexedDB\.open/);
assert.match(persistenceStore, /GRAPH_STORE/);
assert.match(persistenceStore, /SESSION_STORE/);
assert.match(persistenceStore, /META_STORE/);
assert.equal(persistenceStore.includes("localStorage"), false);
assert.equal(persistenceStore.includes("node:"), false);
assert.match(persistenceModel, /draftLinks:\s*\[\]/);
assert.equal(persistenceModel.includes("bearerToken"), false);
assert.equal(persistenceModel.includes("executionResult"), false);
assert.match(styles, /\.file-menu-popover/);
assert.match(styles, /\.header-overflow-menu/);
assert.match(styles, /\.header-overflow-popover/);
assert.match(styles, /\.desktop-history-actions/);
assert.match(styles, /\.inspector-header-action/);
assert.match(headerOverflow, /jaJP\.headerOverflow\.moreEditorActions/);
assert.match(headerOverflow, /event\.key === "Escape"/);
assert.match(headerOverflow, /jaJP\.common\.actions\.undo/);
assert.match(headerOverflow, /jaJP\.common\.actions\.redo/);
assert.match(headerOverflow, /jaJP\.headerOverflow\.hideInspector/);
assert.match(headerOverflow, /jaJP\.headerOverflow\.showInspector/);
assert.match(app, /HeaderOverflowMenu/);
assert.match(app, /className="tertiary-action inspector-header-action"/);

assert.match(styles, /\.primary-action/);
assert.match(styles, /\.execution-panel/);
assert.match(app, /jaJP\.app\.plan/);
assert.match(app, /jaJP\.app\.run/);
assert.match(app, /className="primary-action"/);
assert.match(app, /planCurrentGraph/);
assert.match(app, /runCurrentGraph/);
assert.match(app, /currentValidationIssues\.length > 0/);
assert.match(execution, /jaJP\.execution\.runtimeExecution/);
assert.match(execution, /type="password"/);
assert.match(execution, /jaJP\.execution\.hostAuthorityHint/);
assert.match(execution, /disabled=\{phase === "running"\}/);
assert.match(execution, /type="checkbox"/);
assert.match(execution, /role="alert"/);
assert.match(execution, /groupRequirements/);
assert.match(execution, /event\.isComposing/);
assert.match(execution, /jaJP\.execution\.retryPlan/);
assert.match(execution, /jaJP\.execution\.workingDirectory/);
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

assert.match(recovery, /jaJP\.recovery\.manualRecovery/);
assert.match(recovery, /type="radio"/);
assert.match(recovery, /jaJP\.recovery\.applyAndReplan/);
assert.match(recovery, /jaJP\.recovery\.observedImpactHint/);
assert.match(recovery, /jaJP\.recovery\.runtimeNotExecutable/);
assert.match(recovery, /jaJP\.recovery\.marketplaceDisconnected/);
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
assert.match(canvas, /jaJP\.authoring\.canvas\.observedBlocks/);
assert.match(traceDetail, /jaJP\.trace\.observedRun/);
assert.match(traceDetail, /jaJP\.trace\.observedRoute/);
assert.match(traceDetail, /stderr/);
assert.match(traceDetail, /jaJP\.trace\.evidenceOnlyHint/);
assert.match(traceUnmapped, /jaJP\.trace\.unmappedOriginCount/);
assert.match(traceUnmapped, /jaJP\.trace\.unmappedHint/);
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
    browser_file_menu: "pass",
    narrow_header_overflow: "pass",
    indexeddb_document_session_separation: "pass",
    settled_viewport_persistence: "pass",
    transient_runtime_state_not_persisted: "pass",
  }),
);
