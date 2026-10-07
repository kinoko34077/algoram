import { useCallback, useEffect, useMemo, useState } from "react";
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
  graphIsDirty,
  redoGraph,
  setGraphLabel,
  undoGraph,
  type GraphHistory,
} from "./authoring";
import { demoBundle } from "./fixture";
import { DraftLinkPanel } from "./DraftLinkPanel";
import { GraphAuthoringPanel } from "./GraphAuthoringPanel";
import { GraphCanvas } from "./GraphCanvas";
import { validateGraph, type GraphValidationIssue } from "./graphValidation";
import {
  buildNavigationIndex,
  resolveBlockPath,
  type NavigationRecord,
} from "./navigation";
import {
  addDraftLink,
  EMPTY_GRAPH_PRESENTATION,
  validateDraftLink,
  type DraftLink,
  type GraphPresentationState,
} from "./presentation";
import { markEditorPerformance } from "./perfMarks";
import { SearchPanel } from "./SearchPanel";
import { SourcePanel } from "./SourcePanel";

interface Breadcrumb {
  graphId: string;
  label: string;
}

type PresentationByGraph = Record<string, GraphPresentationState>;
type SelectionByGraph = Record<string, string | null>;

interface GraphFocusRequest {
  graphId: string;
  blockId: string;
  revision: number;
}

function graphLabel(graphId: string): string {
  return demoBundle.graphs[graphId]?.label ?? graphId;
}

export function App() {
  const [path, setPath] = useState<Breadcrumb[]>([
    {
      graphId: demoBundle.rootGraphId,
      label: graphLabel(demoBundle.rootGraphId),
    },
  ]);
  const [selectionByGraph, setSelectionByGraph] = useState<SelectionByGraph>(
    {},
  );
  const [presentationByGraph, setPresentationByGraph] =
    useState<PresentationByGraph>({});
  const [navigationStatus, setNavigationStatus] = useState<string | null>(null);
  const [annotations, setAnnotations] = useState<BlockAnnotations>({});
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [focusRequest, setFocusRequest] = useState<GraphFocusRequest | null>(
    null,
  );
  const [authoringByGraph, setAuthoringByGraph] = useState(() =>
    createAuthoringHistories(demoBundle),
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
      graphs: {
        ...demoBundle.graphs,
        ...workingGraphs,
      },
    };
  }, [authoringByGraph]);

  const navigationIndex = useMemo(
    () => buildNavigationIndex(editorBundle),
    [editorBundle],
  );

  const currentGraphId = path.at(-1)?.graphId ?? demoBundle.rootGraphId;
  const currentHistory = authoringByGraph[currentGraphId] ?? null;
  const currentGraph = editorBundle.graphs[currentGraphId];

  if (!currentGraph) {
    throw new Error(`Missing graph fixture: ${currentGraphId}`);
  }

  const selectedBlockId = selectionByGraph[currentGraphId] ?? null;
  const currentPresentation =
    presentationByGraph[currentGraphId] ?? EMPTY_GRAPH_PRESENTATION;
  const currentGraphDirty = currentHistory ? graphIsDirty(currentHistory) : false;
  const currentValidationIssues = useMemo(
    () => validateGraph(currentGraph),
    [currentGraph],
  );

  const selectedBlock = useMemo(
    () =>
      currentGraph.blocks.find((block) => block.id === selectedBlockId) ?? null,
    [currentGraph, selectedBlockId],
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

      setAuthoringByGraph((current) => ({
        ...current,
        [currentGraphId]: nextHistory,
      }));

      if (currentGraphId === demoBundle.rootGraphId) {
        const nextLabel =
          nextHistory.present.graph.label ?? nextHistory.present.graph.id;
        setPath((current) =>
          current.map((entry, index) =>
            index === 0 ? { ...entry, label: nextLabel } : entry,
          ),
        );
      }
    },
    [currentGraphId, currentHistory],
  );

  const undoCurrentGraph = useCallback(() => {
    if (currentHistory) {
      replaceCurrentHistory(undoGraph(currentHistory));
    }
  }, [currentHistory, replaceCurrentHistory]);

  const redoCurrentGraph = useCallback(() => {
    if (currentHistory) {
      replaceCurrentHistory(redoGraph(currentHistory));
    }
  }, [currentHistory, replaceCurrentHistory]);

  useEffect(() => {
    function textEditorOwnsUndo(target: EventTarget | null): boolean {
      return (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      );
    }

    function handleHistoryShortcut(event: KeyboardEvent) {
      if (
        !currentHistory ||
        textEditorOwnsUndo(event.target) ||
        !(event.metaKey || event.ctrlKey)
      ) {
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
    }

    window.addEventListener("keydown", handleHistoryShortcut);
    return () => window.removeEventListener("keydown", handleHistoryShortcut);
  }, [currentHistory, redoCurrentGraph, undoCurrentGraph]);

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

  const updateCurrentPresentation = useCallback(
    (update: (current: GraphPresentationState) => GraphPresentationState) =>
      updatePresentation(currentGraphId, update),
    [currentGraphId, updatePresentation],
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
  }

  function openGraph(graphId: string, viaBlock: AlgoramBlock) {
    openGraphWithLabel(graphId, viaBlock.label);
  }

  function jumpTo(index: number) {
    setPath((current) => current.slice(0, index + 1));
    setNavigationStatus(null);
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

    if (!window.confirm("Clear this local note? This cannot be undone.")) {
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
        `Cannot reach ${record.label} from the loaded root hierarchy.`,
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

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="title-block">
          <p className="eyebrow">ALGOram</p>
          <h1>{currentGraph.label ?? currentGraph.id}</h1>
        </div>

        <div className="header-actions">
          <div className="header-stat" aria-label="Visible Blocks">
            <strong>{currentGraph.blocks.length}</strong>
            <span>Blocks</span>
          </div>
          <div className="header-editor-status" aria-live="polite">
            <span>{currentHistory ? "Editable" : "Read only"}</span>
            {currentHistory ? (
              <span>{currentGraphDirty ? "Modified" : "Clean"}</span>
            ) : null}
          </div>
          {currentHistory ? (
            <div className="history-actions" aria-label="Graph history">
              <button
                type="button"
                className="tertiary-action"
                onClick={undoCurrentGraph}
                disabled={currentHistory.past.length === 0}
              >
                Undo
              </button>
              <button
                type="button"
                className="tertiary-action"
                onClick={redoCurrentGraph}
                disabled={currentHistory.future.length === 0}
              >
                Redo
              </button>
            </div>
          ) : null}
          <button
            type="button"
            className="tertiary-action"
            aria-controls="inspector-panel"
            aria-expanded={inspectorOpen}
            aria-pressed={inspectorOpen}
            onClick={() => setInspectorOpen((open) => !open)}
          >
            Inspector
          </button>
        </div>
      </header>

      <nav className="breadcrumbs" aria-label="Graph hierarchy">
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

      <SearchPanel
        index={navigationIndex}
        status={navigationStatus}
        onSelectResult={jumpToSearchResult}
      />

      <main
        className={
          inspectorOpen ? "workspace inspector-open" : "workspace inspector-closed"
        }
      >
        <section className="graph-region" aria-label="Graph editor">
          <GraphCanvas
            graph={currentGraph}
            selectedBlockId={selectedBlockId}
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
            onSelectBlock={selectBlock}
            onOpenGraph={openGraph}
          />
        </section>

        {inspectorOpen ? (
          <aside
            id="inspector-panel"
            className="inspector-region"
            aria-label="Block inspector"
          >
            <GraphAuthoringPanel
              graph={currentGraph}
              editable={currentHistory !== null}
              dirty={currentGraphDirty}
              validationIssues={currentValidationIssues}
              onCommitLabel={commitCurrentGraphLabel}
            />

            <section className="block-inspector">
              {selectedBlock ? (
                <>
                  <div className="panel-heading-row">
                    <div>
                      <p className="eyebrow">Selected Block</p>
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
                        Open graph
                      </button>
                    ) : null}
                    {selectedRouteGraphId ? (
                      <button
                        type="button"
                        className="secondary-action"
                        onClick={() =>
                          openGraphWithLabel(
                            selectedRouteGraphId,
                            "Actual selected route",
                          )
                        }
                      >
                        Route
                      </button>
                    ) : null}
                  </div>

                  <DraftLinkPanel
                    graph={currentGraph}
                    selectedBlock={selectedBlock}
                    presentation={currentPresentation}
                    onAdd={addDraftFromInspector}
                  />

                  <AnnotationPanel
                    blockId={selectedBlock.id}
                    value={selectedAnnotation}
                    onChange={updateSelectedAnnotation}
                    onRemove={removeSelectedAnnotation}
                  />
                </>
              ) : (
                <p className="hint">
                  Select a Block to inspect properties, source, or create a
                  presentation-only draft link.
                </p>
              )}
            </section>

            <SourcePanel
              bundle={editorBundle}
              graph={currentGraph}
              selectedBlock={selectedBlock}
              onSelectBlock={selectBlock}
            />
          </aside>
        ) : null}
      </main>
    </div>
  );
}
