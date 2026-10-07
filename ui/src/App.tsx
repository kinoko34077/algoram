import type { XYPosition } from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AlgoramBlock } from "./algoram";
import {
  getBlockAnnotation,
  removeBlockAnnotation,
  setBlockAnnotation,
  type BlockAnnotations,
} from "./annotations";
import { AnnotationPanel } from "./AnnotationPanel";
import {
  applyGraph,
  createAuthoringHistories,
  createGraphHistory,
  graphFingerprint,
  graphIsDirty,
  markGraphSaved,
  redoGraph,
  setGraphLabel,
  undoGraph,
  type GraphHistory,
} from "./authoring";
import {
  collectReusableBlockTemplates,
  instantiateReusableBlock,
  removeBlock,
  type ReusableBlockTemplate,
} from "./blockAuthoring";
import {
  createConnection,
  removeConnection,
  validateConnectionCandidate,
} from "./connectionAuthoring";
import { requestCanonicalGraphDownload } from "./browserDownload";
import {
  loadLastLocalDocument,
  saveLocalDocument,
} from "./browserPersistence";
import {
  createPersistedEditorSession,
  restoreEditorSession,
} from "./documentPersistence";
import { demoBundle } from "./fixture";
import {
  FileMenu,
  type FileOperationStatus,
} from "./FileMenu";
import { readCanonicalGraphFile } from "./graphImport";
import { DraftLinkPanel } from "./DraftLinkPanel";
import {
  ExecutionPanel,
  type ExecutionPhase,
} from "./ExecutionPanel";
import {
  RecoveryPanel,
  type RecoveryDiscoveryPhase,
} from "./RecoveryPanel";
import { GraphAuthoringPanel } from "./GraphAuthoringPanel";
import { GraphCanvas } from "./GraphCanvas";
import { HeaderOverflowMenu } from "./HeaderOverflowMenu";
import { validateGraph, type GraphValidationIssue } from "./graphValidation";
import {
  buildNavigationIndex,
  resolveBlockPath,
  type NavigationRecord,
} from "./navigation";
import {
  addDraftLink,
  EMPTY_GRAPH_PRESENTATION,
  setNodePosition,
  validateDraftLink,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";
import { markEditorPerformance } from "./perfMarks";
import {
  hasExecutableRecoverySelection,
  planGraph,
  recoveryOptions as requestRecoveryOptions,
  recoveryPlanGraph,
  recoveryRunGraph,
  requiredImplementationRefs,
  runGraph,
  type PlanResponse,
  type RecoveryOptionsResponse,
  type RecoverySelections,
  type RunResponse,
  type RuntimeBridgeSettings,
} from "./runtimeBridge";
import { SearchPanel } from "./SearchPanel";
import { SourcePanel } from "./SourcePanel";
import { TraceDetailPanel } from "./TraceDetailPanel";
import { TraceUnmappedPanel } from "./TraceUnmappedPanel";
import { projectExecutionTrace } from "./traceProjection";
import { jaJP } from "./locales/ja-JP";

interface Breadcrumb {
  graphId: string;
  label: string;
}

type PresentationByGraph = Record<string, GraphPresentationState>;
type SelectionByGraph = Record<string, string | null>;
type ConnectionSelectionByGraph = Record<string, string | null>;

interface GraphFocusRequest {
  graphId: string;
  blockId: string;
  revision: number;
}

function graphLabel(graphId: string): string {
  return demoBundle.graphs[graphId]?.label ?? graphId;
}

function textEditorOwnsKeys(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

export function App() {
  const [activeRootGraphId, setActiveRootGraphId] = useState(
    demoBundle.rootGraphId,
  );
  const [persistenceReady, setPersistenceReady] = useState(false);
  const [documentRevision, setDocumentRevision] = useState(0);
  const [path, setPath] = useState<Breadcrumb[]>([
    {
      graphId: demoBundle.rootGraphId,
      label: graphLabel(demoBundle.rootGraphId),
    },
  ]);
  const [selectionByGraph, setSelectionByGraph] = useState<SelectionByGraph>(
    {},
  );
  const [connectionSelectionByGraph, setConnectionSelectionByGraph] =
    useState<ConnectionSelectionByGraph>({});
  const [presentationByGraph, setPresentationByGraph] =
    useState<PresentationByGraph>({});
  const [navigationStatus, setNavigationStatus] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const searchButtonRef = useRef<HTMLButtonElement>(null);
  const [authoringStatus, setAuthoringStatus] = useState<string | null>(null);
  const [fileStatus, setFileStatus] = useState<FileOperationStatus | null>(null);
  const [bridgeSettings, setBridgeSettings] = useState<RuntimeBridgeSettings>({
    baseUrl: "http://127.0.0.1:39091",
    bearerToken: "",
  });
  const [executionOpen, setExecutionOpen] = useState(false);
  const [executionPhase, setExecutionPhase] =
    useState<ExecutionPhase>("idle");
  const [executionPreview, setExecutionPreview] =
    useState<PlanResponse | null>(null);
  const [executionResult, setExecutionResult] =
    useState<RunResponse | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);
  const [grantedImplementationRefs, setGrantedImplementationRefs] = useState<
    Set<string>
  >(() => new Set());
  const executionRequestRevision = useRef(0);
  const executionPlanAbort = useRef<AbortController | null>(null);
  const recoveryRequestRevision = useRef(0);
  const recoveryAbort = useRef<AbortController | null>(null);
  const [recoveryDiscoveryPhase, setRecoveryDiscoveryPhase] =
    useState<RecoveryDiscoveryPhase>("idle");
  const [recoveryOptionsResult, setRecoveryOptionsResult] =
    useState<RecoveryOptionsResponse | null>(null);
  const [recoveryError, setRecoveryError] = useState<string | null>(null);
  const [activeRecoverySelections, setActiveRecoverySelections] =
    useState<RecoverySelections | null>(null);
  const [annotations, setAnnotations] = useState<BlockAnnotations>({});
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [focusRequest, setFocusRequest] = useState<GraphFocusRequest | null>(
    null,
  );
  const [authoringByGraph, setAuthoringByGraph] = useState(() =>
    createAuthoringHistories(demoBundle),
  );

  const blockTemplates = useMemo(
    () => collectReusableBlockTemplates(demoBundle),
    [],
  );

  const editorBundle = useMemo(() => {
    const workingGraphs = Object.fromEntries(
      Object.entries(authoringByGraph).map(([graphId, history]) => [
        graphId,
        history.present.graph,
      ]),
    );

    return {
      ...demoBundle,
      rootGraphId: activeRootGraphId,
      graphs: {
        ...demoBundle.graphs,
        ...workingGraphs,
      },
    };
  }, [activeRootGraphId, authoringByGraph]);

  const navigationIndex = useMemo(
    () => buildNavigationIndex(editorBundle),
    [editorBundle],
  );

  const currentGraphId = path.at(-1)?.graphId ?? activeRootGraphId;
  const currentHistory = authoringByGraph[currentGraphId] ?? null;
  const activeRootHistory = authoringByGraph[activeRootGraphId] ?? null;
  const activeDocumentDirty = activeRootHistory
    ? graphIsDirty(activeRootHistory)
    : false;
  const currentGraph = editorBundle.graphs[currentGraphId];

  if (!currentGraph) {
    throw new Error(`Missing graph fixture: ${currentGraphId}`);
  }

  const selectedBlockId = selectionByGraph[currentGraphId] ?? null;
  const selectedConnectionId =
    connectionSelectionByGraph[currentGraphId] ?? null;
  const currentPresentation =
    presentationByGraph[currentGraphId] ?? EMPTY_GRAPH_PRESENTATION;
  const currentGraphDirty = currentHistory ? graphIsDirty(currentHistory) : false;
  const currentValidationIssues = useMemo(
    () => validateGraph(currentGraph),
    [currentGraph],
  );
  const traceProjectionResult = useMemo(
    () =>
      executionResult
        ? projectExecutionTrace(currentGraph, executionResult.trace)
        : null,
    [currentGraph, executionResult],
  );
  const currentTraceProjection =
    traceProjectionResult?.ok === true
      ? traceProjectionResult.projection
      : null;
  const traceProjectionError =
    traceProjectionResult?.ok === false ? traceProjectionResult.reason : null;

  useEffect(() => {
    let cancelled = false;

    void loadLastLocalDocument()
      .then((localDocument) => {
        if (cancelled || !localDocument) {
          return;
        }

        const availableGraphs = {
          ...demoBundle.graphs,
          [localDocument.graph.id]: localDocument.graph,
        };
        const restored = restoreEditorSession(
          localDocument.graph,
          availableGraphs,
          localDocument.session,
        );

        setActiveRootGraphId(localDocument.graph.id);
        setAuthoringByGraph((current) => ({
          ...current,
          [localDocument.graph.id]: createGraphHistory(localDocument.graph),
        }));
        setPath(
          restored.graphPath.map((graphId) => ({
            graphId,
            label:
              availableGraphs[graphId]?.label ??
              availableGraphs[graphId]?.id ??
              graphId,
          })),
        );
        setPresentationByGraph(restored.presentations);
        setSelectionByGraph(restored.selectedBlockIds);
        setConnectionSelectionByGraph(restored.selectedConnectionIds);
        setInspectorOpen(restored.inspectorOpen);
        setDocumentRevision((revision) => revision + 1);
        setFileStatus({
          kind: "success",
          message: jaJP.app.status.restoredLocalGraph,
        });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFileStatus({
            kind: "error",
            message:
              error instanceof Error
                ? jaJP.app.status.localRestoreUnavailableWithError.replace("{error}", error.message)
                : jaJP.app.status.localRestoreUnavailable,
          });
        }
      })
      .finally(() => {
        if (!cancelled) {
          setPersistenceReady(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!activeDocumentDirty) {
      return;
    }

    function warnBeforeUnload(event: BeforeUnloadEvent) {
      event.preventDefault();
      event.returnValue = "";
    }

    window.addEventListener("beforeunload", warnBeforeUnload);
    return () => window.removeEventListener("beforeunload", warnBeforeUnload);
  }, [activeDocumentDirty]);

  useEffect(() => {
    executionPlanAbort.current?.abort();
    executionPlanAbort.current = null;
    executionRequestRevision.current += 1;
    recoveryAbort.current?.abort();
    recoveryAbort.current = null;
    recoveryRequestRevision.current += 1;
    setExecutionPhase("idle");
    setExecutionPreview(null);
    setExecutionResult(null);
    setExecutionError(null);
    setGrantedImplementationRefs(new Set());
    setRecoveryDiscoveryPhase("idle");
    setRecoveryOptionsResult(null);
    setRecoveryError(null);
    setActiveRecoverySelections(null);
  }, [currentGraphId, currentHistory?.present.revision, documentRevision]);

  const selectedBlock = useMemo(
    () =>
      currentGraph.blocks.find((block) => block.id === selectedBlockId) ?? null,
    [currentGraph, selectedBlockId],
  );
  const selectedTraceObservation =
    selectedBlockId && currentTraceProjection
      ? currentTraceProjection.byBlockId[selectedBlockId]
      : undefined;
  const selectedConnection = useMemo(
    () =>
      currentGraph.connections.find(
        (connection) => connection.id === selectedConnectionId,
      ) ?? null,
    [currentGraph, selectedConnectionId],
  );

  const selectedRouteGraphId =
    selectedBlockId === null
      ? undefined
      : editorBundle.routeInspections?.[selectedBlockId];

  const selectedAnnotation =
    selectedBlock === null
      ? ""
      : getBlockAnnotation(annotations, selectedBlock.id);

  const replaceCurrentHistory = useCallback(
    (nextHistory: GraphHistory) => {
      if (!currentHistory || nextHistory === currentHistory) {
        return;
      }

      setFileStatus(null);
      setAuthoringByGraph((current) => ({
        ...current,
        [currentGraphId]: nextHistory,
      }));

      if (currentGraphId === activeRootGraphId) {
        const nextLabel =
          nextHistory.present.graph.label ?? nextHistory.present.graph.id;
        setPath((current) =>
          current.map((entry, index) =>
            index === 0 ? { ...entry, label: nextLabel } : entry,
          ),
        );
      }
    },
    [activeRootGraphId, currentGraphId, currentHistory],
  );

  const undoCurrentGraph = useCallback(() => {
    if (currentHistory) {
      replaceCurrentHistory(undoGraph(currentHistory));
      setAuthoringStatus(jaJP.app.status.undidCanonicalGraphChange);
    }
  }, [currentHistory, replaceCurrentHistory]);

  const redoCurrentGraph = useCallback(() => {
    if (currentHistory) {
      replaceCurrentHistory(redoGraph(currentHistory));
      setAuthoringStatus(jaJP.app.status.redidCanonicalGraphChange);
    }
  }, [currentHistory, replaceCurrentHistory]);

  const removeSelectedBlock = useCallback(() => {
    if (!currentHistory || !selectedBlock) {
      return;
    }

    const result = removeBlock(currentHistory.present.graph, selectedBlock.id);
    if (!result.ok) {
      setAuthoringStatus(result.reason);
      return;
    }

    const issues = validateGraph(result.graph);
    if (issues.length > 0) {
      setAuthoringStatus(issues[0]?.message ?? jaJP.app.status.graphValidationFailed);
      return;
    }

    replaceCurrentHistory(applyGraph(currentHistory, result.graph));
    setSelectionByGraph((current) => ({
      ...current,
      [currentGraphId]: null,
    }));
    setAuthoringStatus(jaJP.app.status.removedBlock.replace("{label}", result.removed.label));
  }, [currentGraphId, currentHistory, replaceCurrentHistory, selectedBlock]);

  const removeSelectedConnection = useCallback(() => {
    if (!currentHistory || !selectedConnection) {
      return;
    }

    const result = removeConnection(
      currentHistory.present.graph,
      selectedConnection.id,
    );
    if (!result.ok) {
      setAuthoringStatus(result.reason);
      return;
    }

    replaceCurrentHistory(applyGraph(currentHistory, result.graph));
    setConnectionSelectionByGraph((current) => ({
      ...current,
      [currentGraphId]: null,
    }));
    setAuthoringStatus(
      jaJP.app.status.removedConnection.replace("{connectionId}", result.removed.id),
    );
  }, [
    currentGraphId,
    currentHistory,
    replaceCurrentHistory,
    selectedConnection,
  ]);

  useEffect(() => {
    function handleEditorShortcut(event: KeyboardEvent) {
      if (textEditorOwnsKeys(event.target)) {
        return;
      }

      if (event.metaKey || event.ctrlKey) {
        if (!currentHistory) {
          return;
        }

        const key = event.key.toLowerCase();
        if (key === "z") {
          event.preventDefault();
          if (event.shiftKey) {
            redoCurrentGraph();
          } else {
            undoCurrentGraph();
          }
        } else if (key === "y" && !event.shiftKey) {
          event.preventDefault();
          redoCurrentGraph();
        }
        return;
      }

      if (
        currentHistory &&
        (event.key === "Delete" || event.key === "Backspace")
      ) {
        if (selectedConnection) {
          event.preventDefault();
          removeSelectedConnection();
        } else if (selectedBlock) {
          event.preventDefault();
          removeSelectedBlock();
        }
      }
    }

    window.addEventListener("keydown", handleEditorShortcut);
    return () => window.removeEventListener("keydown", handleEditorShortcut);
  }, [
    currentHistory,
    redoCurrentGraph,
    removeSelectedBlock,
    removeSelectedConnection,
    selectedBlock,
    selectedConnection,
    undoCurrentGraph,
  ]);

  const updatePresentation = useCallback(
    (
      graphId: string,
      update: (current: GraphPresentationState) => GraphPresentationState,
    ) => {
      markEditorPerformance("presentation-commit");
      setPresentationByGraph((current) => {
        const previous = current[graphId] ?? EMPTY_GRAPH_PRESENTATION;
        const next = update(previous);
        if (next === previous) {
          return current;
        }

        return {
          ...current,
          [graphId]: next,
        };
      });
    },
    [],
  );

  const selectBlock = useCallback(
    (blockId: string | null) => {
      if (blockId !== null) {
        setInspectorOpen(true);
        setConnectionSelectionByGraph((current) => ({
          ...current,
          [currentGraphId]: null,
        }));
      }

      setSelectionByGraph((current) => {
        const previous = current[currentGraphId] ?? null;
        if (previous === blockId) {
          return current;
        }

        return {
          ...current,
          [currentGraphId]: blockId,
        };
      });
    },
    [currentGraphId],
  );

  const selectConnection = useCallback(
    (connectionId: string | null) => {
      if (connectionId !== null) {
        setInspectorOpen(true);
        setSelectionByGraph((current) => ({
          ...current,
          [currentGraphId]: null,
        }));
      }

      setConnectionSelectionByGraph((current) => {
        const previous = current[currentGraphId] ?? null;
        if (previous === connectionId) {
          return current;
        }

        return {
          ...current,
          [currentGraphId]: connectionId,
        };
      });
    },
    [currentGraphId],
  );

  const updateCurrentPresentation = useCallback(
    (update: (current: GraphPresentationState) => GraphPresentationState) =>
      updatePresentation(currentGraphId, update),
    [currentGraphId, updatePresentation],
  );

  const addReusableBlock = useCallback(
    (template: ReusableBlockTemplate, position: XYPosition) => {
      if (!currentHistory) {
        return;
      }

      const result = instantiateReusableBlock(
        currentHistory.present.graph,
        template,
      );
      const issues = validateGraph(result.graph);
      if (issues.length > 0) {
        setAuthoringStatus(issues[0]?.message ?? "Graph validation failed.");
        return;
      }

      replaceCurrentHistory(applyGraph(currentHistory, result.graph));
      updatePresentation(currentGraphId, (current) =>
        setNodePosition(current, result.block.id, position),
      );
      setSelectionByGraph((current) => ({
        ...current,
        [currentGraphId]: result.block.id,
      }));
      setAuthoringStatus(jaJP.app.status.addedBlock.replace("{label}", result.block.label));
    },
    [
      currentGraphId,
      currentHistory,
      replaceCurrentHistory,
      updatePresentation,
    ],
  );

  function openGraphWithLabel(graphId: string, label: string) {
    if (!editorBundle.graphs[graphId]) {
      return;
    }

    setPath((current) => [
      ...current,
      {
        graphId,
        label,
      },
    ]);
    setNavigationStatus(null);
    setAuthoringStatus(null);
  }

  function openGraph(graphId: string, viaBlock: AlgoramBlock) {
    openGraphWithLabel(graphId, viaBlock.label);
  }

  function jumpTo(index: number) {
    setPath((current) => current.slice(0, index + 1));
    setNavigationStatus(null);
    setAuthoringStatus(null);
  }

  function updateSelectedAnnotation(value: string) {
    if (!selectedBlock) {
      return;
    }

    setAnnotations((current) =>
      setBlockAnnotation(current, selectedBlock.id, value),
    );
  }

  function removeSelectedAnnotation() {
    if (!selectedBlock || selectedAnnotation.length === 0) {
      return;
    }

    if (!window.confirm(jaJP.app.status.clearLocalNoteConfirm)) {
      return;
    }

    setAnnotations((current) =>
      removeBlockAnnotation(current, selectedBlock.id),
    );
  }

  function jumpToSearchResult(record: NavigationRecord) {
    const resolvedPath = resolveBlockPath(navigationIndex, record);
    if (!resolvedPath) {
      setNavigationStatus(
        jaJP.app.status.cannotReachFromRoot.replace("{label}", record.label),
      );
      return;
    }

    setPath(
      resolvedPath.map((entry) => ({
        graphId: entry.graphId,
        label: entry.label,
      })),
    );
    setSelectionByGraph((current) => ({
      ...current,
      [record.graphId]: record.blockId,
    }));
    setFocusRequest((current) => ({
      graphId: record.graphId,
      blockId: record.blockId,
      revision: (current?.revision ?? 0) + 1,
    }));
    setNavigationStatus(null);
    setAuthoringStatus(null);
  }

  function commitCurrentGraphLabel(label: string): GraphValidationIssue[] {
    if (!currentHistory) {
      return [];
    }

    const nextGraph = setGraphLabel(currentHistory.present.graph, label);
    const issues = validateGraph(nextGraph);
    if (issues.length > 0) {
      return issues;
    }

    replaceCurrentHistory(applyGraph(currentHistory, nextGraph));
    return [];
  }

  const validateCanonicalLink = useCallback(
    (link: DraftLink): string | null => {
      if (!currentHistory) {
        return jaJP.app.status.graphReadOnly;
      }

      return validateConnectionCandidate(currentHistory.present.graph, {
        sourceBlockId: link.sourceBlockId,
        sourcePortId: link.sourcePortId,
        targetBlockId: link.targetBlockId,
        targetPortId: link.targetPortId,
      });
    },
    [currentHistory],
  );

  const addCanonicalConnection = useCallback(
    (link: DraftLink): string | null => {
      if (!currentHistory) {
        return jaJP.app.status.graphReadOnly;
      }

      const result = createConnection(currentHistory.present.graph, {
        sourceBlockId: link.sourceBlockId,
        sourcePortId: link.sourcePortId,
        targetBlockId: link.targetBlockId,
        targetPortId: link.targetPortId,
      });
      if (!result.ok) {
        setAuthoringStatus(result.reason);
        return result.reason;
      }

      replaceCurrentHistory(applyGraph(currentHistory, result.graph));
      setSelectionByGraph((current) => ({
        ...current,
        [currentGraphId]: null,
      }));
      setConnectionSelectionByGraph((current) => ({
        ...current,
        [currentGraphId]: result.connection.id,
      }));
      setAuthoringStatus(
        jaJP.app.status.createdConnection.replace("{connectionId}", result.connection.id),
      );
      return null;
    },
    [currentGraphId, currentHistory, replaceCurrentHistory],
  );

  const requiredExecutionRefs = useMemo(
    () =>
      executionPreview
        ? requiredImplementationRefs(executionPreview)
        : [],
    [executionPreview],
  );
  const allExecutionGrantsApproved =
    requiredExecutionRefs.length > 0 &&
    requiredExecutionRefs.every((implementationRef) =>
      grantedImplementationRefs.has(implementationRef),
    );

  const clearRecoveryContext = useCallback(() => {
    recoveryAbort.current?.abort();
    recoveryAbort.current = null;
    recoveryRequestRevision.current += 1;
    setRecoveryDiscoveryPhase("idle");
    setRecoveryOptionsResult(null);
    setRecoveryError(null);
    setActiveRecoverySelections(null);
  }, []);

  const dismissExecutionPreview = useCallback(() => {
    if (executionPhase === "running") {
      return;
    }
    executionPlanAbort.current?.abort();
    executionPlanAbort.current = null;
    executionRequestRevision.current += 1;
    setExecutionPhase("idle");
    setExecutionPreview(null);
    setExecutionResult(null);
    setExecutionError(null);
    setGrantedImplementationRefs(new Set());
    clearRecoveryContext();
  }, [clearRecoveryContext, executionPhase]);

  const updateBridgeSettings = useCallback(
    (settings: RuntimeBridgeSettings) => {
      executionPlanAbort.current?.abort();
      executionPlanAbort.current = null;
      executionRequestRevision.current += 1;
      setBridgeSettings(settings);
      setExecutionPhase("idle");
      setExecutionPreview(null);
      setExecutionResult(null);
      setExecutionError(null);
      setGrantedImplementationRefs(new Set());
      clearRecoveryContext();
    },
    [clearRecoveryContext],
  );

  const setExecutionGrant = useCallback(
    (implementationRef: string, granted: boolean) => {
      setGrantedImplementationRefs((current) => {
        const next = new Set(current);
        if (granted) {
          next.add(implementationRef);
        } else {
          next.delete(implementationRef);
        }
        return next;
      });
    },
    [],
  );

  const discoverRecoveryOptions = useCallback(
    async (plan: PlanResponse["plan"], trace: RunResponse["trace"]) => {
      recoveryAbort.current?.abort();
      const controller = new AbortController();
      recoveryAbort.current = controller;
      const requestRevision = recoveryRequestRevision.current + 1;
      recoveryRequestRevision.current = requestRevision;
      setRecoveryDiscoveryPhase("loading");
      setRecoveryOptionsResult(null);
      setRecoveryError(null);

      try {
        const options = await requestRecoveryOptions(
          bridgeSettings,
          currentGraph,
          plan,
          trace,
          controller.signal,
        );
        if (recoveryRequestRevision.current !== requestRevision) {
          return;
        }
        setRecoveryOptionsResult(options);
        setRecoveryDiscoveryPhase("ready");
      } catch (error: unknown) {
        if (recoveryRequestRevision.current !== requestRevision) {
          return;
        }
        if (error instanceof DOMException && error.name === "AbortError") {
          setRecoveryDiscoveryPhase("idle");
          return;
        }
        setRecoveryDiscoveryPhase("failed");
        setRecoveryError(
          error instanceof Error
            ? error.message
            : jaJP.app.status.recoveryDiscoveryFailed,
        );
      } finally {
        if (recoveryAbort.current === controller) {
          recoveryAbort.current = null;
        }
      }
    },
    [bridgeSettings, currentGraph],
  );

  const applyRecoverySelections = useCallback(
    async (selections: RecoverySelections) => {
      if (!hasExecutableRecoverySelection(selections)) {
        return;
      }

      executionPlanAbort.current?.abort();
      const controller = new AbortController();
      executionPlanAbort.current = controller;
      const requestRevision = executionRequestRevision.current + 1;
      executionRequestRevision.current = requestRevision;
      const previousPreview = executionPreview;

      setExecutionError(null);
      setRecoveryError(null);
      setExecutionPhase("planning");

      try {
        const preview = await recoveryPlanGraph(
          bridgeSettings,
          currentGraph,
          selections,
          controller.signal,
        );
        if (executionRequestRevision.current !== requestRevision) {
          return;
        }

        const sameAccess =
          previousPreview !== null &&
          JSON.stringify(previousPreview.access_report.requirements) ===
            JSON.stringify(preview.access_report.requirements);
        const requiredRefs = new Set(requiredImplementationRefs(preview));
        setGrantedImplementationRefs((current) =>
          sameAccess
            ? new Set([...current].filter((ref) => requiredRefs.has(ref)))
            : new Set(),
        );
        setExecutionPreview(preview);
        setActiveRecoverySelections(structuredClone(selections));
        setExecutionPhase("ready");
        setRecoveryDiscoveryPhase("idle");
        setRecoveryOptionsResult(null);
        setRecoveryError(null);
      } catch (error: unknown) {
        if (executionRequestRevision.current !== requestRevision) {
          return;
        }
        if (error instanceof DOMException && error.name === "AbortError") {
          setExecutionPhase("failed");
          return;
        }

        setExecutionPhase("failed");
        const message =
          error instanceof Error ? error.message : jaJP.app.status.recoveryReplanFailed;
        setRecoveryError(message);
        setExecutionError(jaJP.app.status.recoveryReplanBlocked.replace("{message}", message));
      } finally {
        if (executionPlanAbort.current === controller) {
          executionPlanAbort.current = null;
        }
      }
    },
    [bridgeSettings, currentGraph, executionPreview],
  );

  const planCurrentGraph = useCallback(async () => {
    setInspectorOpen(true);
    setExecutionResult(null);
    clearRecoveryContext();

    if (currentValidationIssues.length > 0) {
      executionPlanAbort.current?.abort();
      executionPlanAbort.current = null;
      setExecutionPreview(null);
      setGrantedImplementationRefs(new Set());
      setExecutionPhase("failed");
      setExecutionError(
        jaJP.app.status.localGraphValidationFailed.replace("{error}", currentValidationIssues[0]?.message ?? "invalid Graph"),
      );
      return;
    }

    executionPlanAbort.current?.abort();
    const controller = new AbortController();
    executionPlanAbort.current = controller;

    const previousPreview = executionPreview;
    const requestRevision = executionRequestRevision.current + 1;
    executionRequestRevision.current = requestRevision;
    setExecutionPreview(null);
    setExecutionError(null);
    setExecutionPhase("planning");

    try {
      const preview = await planGraph(
        bridgeSettings,
        currentGraph,
        controller.signal,
      );
      if (executionRequestRevision.current !== requestRevision) {
        return;
      }

      const samePlan =
        previousPreview !== null &&
        JSON.stringify(previousPreview.plan) === JSON.stringify(preview.plan);
      const requiredRefs = new Set(requiredImplementationRefs(preview));
      setGrantedImplementationRefs((current) =>
        samePlan
          ? new Set([...current].filter((ref) => requiredRefs.has(ref)))
          : new Set(),
      );
      setExecutionPreview(preview);
      setExecutionPhase("ready");
    } catch (error: unknown) {
      if (executionRequestRevision.current !== requestRevision) {
        return;
      }
      if (error instanceof DOMException && error.name === "AbortError") {
        setExecutionPhase("idle");
        setExecutionError(null);
        return;
      }
      setExecutionPhase("failed");
      setExecutionError(
        error instanceof Error ? error.message : jaJP.app.status.runtimePlanningFailed,
      );
    } finally {
      if (executionPlanAbort.current === controller) {
        executionPlanAbort.current = null;
      }
    }
  }, [
    bridgeSettings,
    clearRecoveryContext,
    currentGraph,
    currentValidationIssues,
    executionPreview,
  ]);

  const runCurrentGraph = useCallback(async () => {
    if (!executionPreview || !allExecutionGrantsApproved) {
      return;
    }

    const requestRevision = executionRequestRevision.current + 1;
    executionRequestRevision.current = requestRevision;
    setExecutionResult(null);
    setExecutionError(null);
    setExecutionPhase("running");
    setInspectorOpen(true);

    try {
      const allowedImplementationRefs = [...grantedImplementationRefs].sort();
      const result =
        activeRecoverySelections &&
        hasExecutableRecoverySelection(activeRecoverySelections)
          ? await recoveryRunGraph(
              bridgeSettings,
              currentGraph,
              executionPreview.plan,
              allowedImplementationRefs,
              activeRecoverySelections,
            )
          : await runGraph(
              bridgeSettings,
              currentGraph,
              executionPreview.plan,
              allowedImplementationRefs,
            );
      if (executionRequestRevision.current !== requestRevision) {
        return;
      }

      setExecutionResult(result);
      const succeeded = result.trace.entries.every(
        (entry) => entry.status === "succeeded",
      );
      setExecutionPhase(succeeded ? "succeeded" : "failed");
      if (succeeded) {
        clearRecoveryContext();
      } else {
        setExecutionError(jaJP.app.status.guardedRunFailedStep);
        setActiveRecoverySelections(null);
        void discoverRecoveryOptions(executionPreview.plan, result.trace);
      }
    } catch (error: unknown) {
      if (executionRequestRevision.current !== requestRevision) {
        return;
      }
      setExecutionPhase("failed");
      setExecutionError(
        error instanceof Error ? error.message : jaJP.app.status.guardedRuntimeFailed,
      );
    }
  }, [
    activeRecoverySelections,
    allExecutionGrantsApproved,
    bridgeSettings,
    clearRecoveryContext,
    currentGraph,
    discoverRecoveryOptions,
    executionPreview,
    grantedImplementationRefs,
  ]);

  const saveActiveDocumentLocal = useCallback(async () => {
    const history = authoringByGraph[activeRootGraphId];
    if (!history) {
      setFileStatus({
        kind: "error",
        message: jaJP.app.status.saveEditableOnly,
      });
      return;
    }

    const graphSnapshot = history.present.graph;
    const issues = validateGraph(graphSnapshot);
    if (issues.length > 0) {
      setFileStatus({
        kind: "error",
        message: jaJP.app.status.saveBlocked.replace("{error}", issues[0]?.message ?? jaJP.app.status.graphValidationFailed),
      });
      return;
    }

    const savedFingerprint = graphFingerprint(graphSnapshot);
    const session = createPersistedEditorSession(
      activeRootGraphId,
      editorBundle.graphs,
      path.map((entry) => entry.graphId),
      presentationByGraph,
      selectionByGraph,
      connectionSelectionByGraph,
      inspectorOpen,
    );

    try {
      await saveLocalDocument(graphSnapshot, session);
      setAuthoringByGraph((current) => {
        const latest = current[activeRootGraphId];
        if (
          !latest ||
          latest.present.fingerprint !== savedFingerprint
        ) {
          return current;
        }

        return {
          ...current,
          [activeRootGraphId]: markGraphSaved(latest),
        };
      });
      setFileStatus({
        kind: "success",
        message: jaJP.app.status.savedLocalSnapshot,
      });
    } catch (error: unknown) {
      setFileStatus({
        kind: "error",
        message:
          error instanceof Error
            ? jaJP.app.status.saveLocalFailedWithError.replace("{error}", error.message)
            : jaJP.app.status.saveLocalFailed,
      });
    }
  }, [
    activeRootGraphId,
    authoringByGraph,
    connectionSelectionByGraph,
    editorBundle.graphs,
    inspectorOpen,
    path,
    presentationByGraph,
    selectionByGraph,
  ]);

  const openCanonicalGraphFile = useCallback(
    async (file: File) => {
      if (executionPhase === "planning" || executionPhase === "running") {
        setFileStatus({
          kind: "error",
          message: jaJP.app.status.openUnavailableDuringRuntime,
        });
        return;
      }

      if (
        activeDocumentDirty &&
        !window.confirm(
          jaJP.app.status.discardUnsavedConfirm,
        )
      ) {
        setFileStatus({
          kind: "info",
          message: jaJP.app.status.openCancelled,
        });
        return;
      }

      try {
        const graph = await readCanonicalGraphFile(file);
        const nextHistory = createGraphHistory(graph);

        setActiveRootGraphId(graph.id);
        setAuthoringByGraph((current) => ({
          ...current,
          [graph.id]: nextHistory,
        }));
        setPath([
          {
            graphId: graph.id,
            label: graph.label ?? graph.id,
          },
        ]);
        setPresentationByGraph((current) => {
          const next = { ...current };
          delete next[graph.id];
          return next;
        });
        setSelectionByGraph((current) => ({
          ...current,
          [graph.id]: null,
        }));
        setConnectionSelectionByGraph((current) => ({
          ...current,
          [graph.id]: null,
        }));
        setFocusRequest(null);
        setAuthoringStatus(null);
        setNavigationStatus(null);
        setInspectorOpen(true);
        setDocumentRevision((revision) => revision + 1);
        setFileStatus({
          kind: "success",
          message: jaJP.app.status.openedFile.replace("{fileName}", file.name),
        });
      } catch (error: unknown) {
        setFileStatus({
          kind: "error",
          message:
            error instanceof Error
              ? `Open failed: ${error.message}`
              : "Open failed.",
        });
      }
    },
    [activeDocumentDirty, executionPhase],
  );

  function exportCurrentGraph() {
    try {
      const payload = requestCanonicalGraphDownload(currentGraph);
      setFileStatus({
        kind: "info",
        message: jaJP.app.status.downloadRequested.replace("{fileName}", payload.filename),
      });
    } catch (error: unknown) {
      setFileStatus({
        kind: "error",
        message:
          error instanceof Error
            ? jaJP.app.status.exportBlockedWithError.replace("{error}", error.message)
            : jaJP.app.status.exportBlocked,
      });
    }
  }

  function addDraftFromInspector(link: DraftLink): string | null {
    const error = validateDraftLink(currentGraph, currentPresentation, link);
    if (error) {
      return error;
    }

    updatePresentation(currentGraphId, (current) =>
      addDraftLink(currentGraph, current, link),
    );
    return null;
  }

  if (!persistenceReady) {
    return (
      <div className="startup-status" role="status">
        {jaJP.app.restoringEditor}
      </div>
    );
  }

  return (
    <div className={executionOpen ? "app-shell execution-open" : "app-shell"}>
      <header className="app-header">
        <div className="title-block">
          <p className="eyebrow">{jaJP.app.brand}</p>
          <h1>{currentGraph.label ?? currentGraph.id}</h1>
        </div>

        <div className="header-actions">
          <div className="header-editor-status" aria-live="polite">
            <span>{currentHistory ? jaJP.common.states.editable : jaJP.common.states.readOnly}</span>
            {currentHistory ? (
              <span>{currentGraphDirty ? jaJP.common.states.modified : jaJP.common.states.clean}</span>
            ) : null}
          </div>
          {currentHistory ? (
            <div
              className="history-actions desktop-history-actions"
              aria-label={jaJP.app.graphHistory}
            >
              <button
                type="button"
                className="tertiary-action"
                onClick={undoCurrentGraph}
                disabled={currentHistory.past.length === 0}
              >
                {jaJP.common.actions.undo}
              </button>
              <button
                type="button"
                className="tertiary-action"
                onClick={redoCurrentGraph}
                disabled={currentHistory.future.length === 0}
              >
                {jaJP.common.actions.redo}
              </button>
            </div>
          ) : null}
          <button
            type="button"
            ref={searchButtonRef}
            className="tertiary-action header-search-action"
            aria-controls="editor-search-panel"
            aria-expanded={searchOpen}
            aria-label={jaJP.authoring.search.graphSearch}
            onClick={() => setSearchOpen((open) => !open)}
          >
            {jaJP.app.search}
          </button>
          <button
            type="button"
            className="secondary-action execution-entry-action"
            aria-controls="runtime-execution-panel"
            aria-expanded={executionOpen}
            onClick={() => setExecutionOpen((open) => !open)}
          >
            {jaJP.app.executionEntry}
          </button>
          <FileMenu
            canOpen={
              executionPhase !== "planning" && executionPhase !== "running"
            }
            canSaveLocal={activeRootHistory !== null}
            status={fileStatus}
            onOpenFile={openCanonicalGraphFile}
            onSaveLocal={saveActiveDocumentLocal}
            onExport={exportCurrentGraph}
          />
          <button
            type="button"
            className="tertiary-action inspector-header-action"
            aria-controls="inspector-panel"
            aria-expanded={inspectorOpen}
            aria-pressed={inspectorOpen}
            onClick={() => setInspectorOpen((open) => !open)}
          >
            {jaJP.app.inspector}
          </button>
          <HeaderOverflowMenu
            historyAvailable={currentHistory !== null}
            canUndo={(currentHistory?.past.length ?? 0) > 0}
            canRedo={(currentHistory?.future.length ?? 0) > 0}
            inspectorOpen={inspectorOpen}
            onUndo={undoCurrentGraph}
            onRedo={redoCurrentGraph}
            onToggleInspector={() => setInspectorOpen((open) => !open)}
          />
        </div>
      </header>

      <nav className="breadcrumbs" aria-label={jaJP.app.graphHierarchy}>
        {path.map((entry, index) => (
          <button
            key={`${entry.graphId}:${index}`}
            type="button"
            onClick={() => jumpTo(index)}
            aria-current={index === path.length - 1 ? "page" : undefined}
          >
            {entry.label}
          </button>
        ))}
      </nav>

      <div id="editor-search-panel" className="search-overlay" hidden={!searchOpen}>
        <SearchPanel
          index={navigationIndex}
          status={navigationStatus}
          open={searchOpen}
          onClose={() => {
            setSearchOpen(false);
            searchButtonRef.current?.focus();
          }}
          onSelectResult={(record) => {
            jumpToSearchResult(record);
            setSearchOpen(false);
          }}
        />
      </div>

      <main
        className={
          inspectorOpen ? "workspace inspector-open" : "workspace inspector-closed"
        }
      >
        <section className="graph-region" aria-label={jaJP.app.graphEditor}>
          <GraphCanvas
            graph={currentGraph}
            editable={currentHistory !== null}
            blockTemplates={blockTemplates}
            authoringStatus={authoringStatus}
            traceProjection={currentTraceProjection}
            selectedBlockId={selectedBlockId}
            selectedConnectionId={selectedConnectionId}
            focusRequest={
              focusRequest?.graphId === currentGraphId
                ? {
                    blockId: focusRequest.blockId,
                    revision: focusRequest.revision,
                  }
                : null
            }
            presentation={currentPresentation}
            onPresentationChange={updateCurrentPresentation}
            onAddBlock={addReusableBlock}
            validateCanonicalConnection={validateCanonicalLink}
            onAddConnection={addCanonicalConnection}
            onSelectBlock={selectBlock}
            onSelectConnection={selectConnection}
            onOpenGraph={openGraph}
          />
        </section>

        {inspectorOpen ? (
          <aside
            id="inspector-panel"
            className="inspector-region"
            aria-label={jaJP.app.blockInspector}
          >
            <section className="block-inspector">
              {selectedConnection ? (
                <>
                  <div className="panel-heading-row">
                    <div>
                      <p className="eyebrow">{jaJP.app.selectedConnection}</p>
                      <h2>{selectedConnection.id}</h2>
                      <p className="block-id">
                        {selectedConnection.source.block_id}.
                        {selectedConnection.source.port_id} →{" "}
                        {selectedConnection.target.block_id}.
                        {selectedConnection.target.port_id}
                      </p>
                    </div>
                    <button
                      type="button"
                      className="tertiary-action"
                      onClick={() => selectConnection(null)}
                    >
                      {jaJP.common.actions.clear}
                    </button>
                  </div>

                  {currentHistory ? (
                    <div className="inspector-actions">
                      <button
                        type="button"
                        className="danger-action"
                        onClick={removeSelectedConnection}
                      >
                        {jaJP.app.removeConnection}
                      </button>
                    </div>
                  ) : (
                    <p className="compact-hint">
                      {jaJP.app.connectionReadOnly}
                    </p>
                  )}

                  {authoringStatus ? (
                    <p className="inline-status" role="status">
                      {authoringStatus}
                    </p>
                  ) : null}
                </>
              ) : selectedBlock ? (
                <>
                  <div className="panel-heading-row">
                    <div>
                      <p className="eyebrow">{jaJP.app.selectedBlock}</p>
                      <h2>{selectedBlock.label}</h2>
                      <p className="block-id">{selectedBlock.id}</p>
                    </div>
                    <button
                      type="button"
                      className="tertiary-action"
                      onClick={() => selectBlock(null)}
                    >
                      Clear
                    </button>
                  </div>

                  <div className="inspector-actions">
                    {selectedBlock.internal_graph_ref ? (
                      <button
                        type="button"
                        className="secondary-action"
                        onClick={() =>
                          openGraph(
                            selectedBlock.internal_graph_ref as string,
                            selectedBlock,
                          )
                        }
                      >
                        {jaJP.app.openGraph}
                      </button>
                    ) : null}
                    {selectedRouteGraphId ? (
                      <button
                        type="button"
                        className="secondary-action"
                        onClick={() =>
                          openGraphWithLabel(
                            selectedRouteGraphId,
                            jaJP.app.actualSelectedRoute,
                          )
                        }
                      >
                        {jaJP.app.route}
                      </button>
                    ) : null}
                    {currentHistory ? (
                      <button
                        type="button"
                        className="danger-action"
                        onClick={removeSelectedBlock}
                      >
                        {jaJP.app.removeBlock}
                      </button>
                    ) : null}
                  </div>

                  {authoringStatus ? (
                    <p className="inline-status" role="status">
                      {authoringStatus}
                    </p>
                  ) : null}

                  {selectedTraceObservation ? (
                    <TraceDetailPanel observation={selectedTraceObservation} />
                  ) : null}

                  <DraftLinkPanel
                    graph={currentGraph}
                    selectedBlock={selectedBlock}
                    presentation={currentPresentation}
                    mode={currentHistory ? "canonical" : "draft"}
                    validateCandidate={
                      currentHistory
                        ? validateCanonicalLink
                        : (link) =>
                            validateDraftLink(
                              currentGraph,
                              currentPresentation,
                              link,
                            )
                    }
                    onAdd={
                      currentHistory
                        ? addCanonicalConnection
                        : addDraftFromInspector
                    }
                  />

                  <AnnotationPanel
                    blockId={selectedBlock.id}
                    value={selectedAnnotation}
                    onChange={updateSelectedAnnotation}
                    onRemove={removeSelectedAnnotation}
                  />
                </>
              ) : (
                <>
                  <p className="hint">
                    {jaJP.app.selectBlockHint}
                  </p>
                  {authoringStatus ? (
                    <p className="inline-status" role="status">
                      {authoringStatus}
                    </p>
                  ) : null}
                </>
              )}
            </section>

            <details className="inspector-disclosure">
              <summary>{jaJP.authoring.graphProperties.properties}{currentValidationIssues.length > 0 ? ` · ${jaJP.authoring.graphProperties.issueCount.replace("{count}", String(currentValidationIssues.length))}` : ""}</summary>
            <GraphAuthoringPanel
              graph={currentGraph}
              editable={currentHistory !== null}
              dirty={currentGraphDirty}
              validationIssues={currentValidationIssues}
              onCommitLabel={commitCurrentGraphLabel}
            />
            </details>

            <details className="inspector-disclosure">
              <summary>{jaJP.authoring.sourcePanel.source}</summary>
            <SourcePanel
              bundle={editorBundle}
              graph={currentGraph}
              selectedBlock={selectedBlock}
              onSelectBlock={selectBlock}
            />
            </details>

            <div id="runtime-execution-panel">
              <ExecutionPanel
                graph={currentGraph}
                phase={executionPhase}
                settings={bridgeSettings}
                preview={executionPreview}
                result={executionResult}
                error={executionError}
                grantedImplementationRefs={grantedImplementationRefs}
                onSettingsChange={updateBridgeSettings}
                onGrantChange={setExecutionGrant}
                onRetryPlan={planCurrentGraph}
                onDismiss={dismissExecutionPreview}
              />
              {activeRecoverySelections ? (
                <p className="inline-status recovery-applied-status" role="status">
                  {jaJP.app.recoverySelectionsHint}
                </p>
              ) : null}
              <RecoveryPanel
                options={recoveryOptionsResult}
                phase={recoveryDiscoveryPhase}
                error={recoveryError}
                disabled={
                  executionPhase === "planning" || executionPhase === "running"
                }
                onApply={applyRecoverySelections}
              />
              {currentTraceProjection ? (
                <TraceUnmappedPanel projection={currentTraceProjection} />
              ) : null}
              {traceProjectionError ? (
                <p className="inline-status error-text" role="alert">
                  {jaJP.app.observedTraceHidden.replace("{error}", traceProjectionError)}
                </p>
              ) : null}
            </div>

          </aside>
        ) : null}
      </main>
      {/* U3_EXECUTION_CONTROLS_INSERTION */}
    </div>
  );
}
